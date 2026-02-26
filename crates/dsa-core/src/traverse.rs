use crate::events::{FsEntryKind, OsRawMetadata, TraverseEvent, TraverseErrorKind};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct TraverseConfig {
    pub follow_symlinks: bool,
}

impl Default for TraverseConfig {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
        }
    }
}


fn kind_from_meta(ft: &fs::FileType) -> FsEntryKind {
    if ft.is_file() {
        FsEntryKind::File 
    } else if ft.is_dir() {
        FsEntryKind::Directory
    } else if ft.is_symlink() {
        FsEntryKind::Symlink
    } else {
        FsEntryKind::Other
    }
}

fn raw_metadata(path: &Path, cfg: &TraverseConfig) -> Result<OsRawMetadata, String> {
    let meta = if cfg.follow_symlinks {
        // traverses symlinks when getting metadata.
        fs::metadata(path)
    } else {
        // does not traverse symlinks when getting metadata.
        fs::symlink_metadata(path)
    }
    .map_err(|e| e.to_string())?; // calls unwrap() if it fails e.to_string() is immediately returned as Err(String)

    let ft = meta.file_type();
    let kind = kind_from_meta(&ft);
    let modified: Option<SystemTime> = meta.modified().ok(); // converts to Option, treating errors as None

    let logical_len_bytes = if kind == FsEntryKind::File {
        Some(meta.len())
    } else {
        None
    };

    Ok(OsRawMetadata {
        kind,
        modified,
        logical_len_bytes,
    })
}


pub fn traverse(
    root: impl AsRef<Path>,
    cfg: &TraverseConfig,
) -> impl Iterator<Item = TraverseEvent> {
    use std::collections::VecDeque;

    let root = root.as_ref().to_path_buf();
    let mut stack: Vec<PathBuf> = vec![root];
    let mut out: VecDeque<TraverseEvent> = VecDeque::new();
    let cfg = cfg.clone();

    std::iter::from_fn(move || loop {
        // If we already buffered events (Entry then maybe Errors), emit them first.
        if let Some(ev) = out.pop_front() {
            return Some(ev);
        }

        // Otherwise, take next path to process.
        let path = stack.pop()?;

        match raw_metadata(&path, &cfg) {
            Ok(raw) => {
                // Always buffer the Entry first.
                out.push_back(TraverseEvent::Entry {
                    path: path.clone(),
                    raw: raw.clone(),
                });

                // Then attempt expansion if directory.
                if raw.kind == FsEntryKind::Directory {
                    match fs::read_dir(&path) {
                        Ok(rd) => {
                            for child in rd {
                                match child {
                                    Ok(de) => stack.push(de.path()),
                                    Err(e) => out.push_back(TraverseEvent::Error {
                                        path: path.clone(),
                                        kind: TraverseErrorKind::ReadDirEntryFailed,
                                        message: e.to_string(),
                                    }),
                                }
                            }
                        }
                        Err(e) => out.push_back(TraverseEvent::Error {
                            path: path.clone(),
                            kind: TraverseErrorKind::ExpandFailed,
                            message: e.to_string(),
                        }),
                    }
                }

                // Now loop; next iteration will pop_front() the Entry.
                continue;
            }
            Err(msg) => {
                return Some(TraverseEvent::Error { 
                    path, 
                    kind: TraverseErrorKind::StatFailed,
                    message: msg 
                });
            }
        }
    })
}