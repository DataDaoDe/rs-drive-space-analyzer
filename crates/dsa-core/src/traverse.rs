use crate::events::{FsEntryKind, OsRawMetadata, TraverseEvent, TraverseErrorKind, SkipReason};
use crate::policy::{ChildOrdering, ErrorPolicy, ErrorReaction};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct TraverseConfig {
    pub follow_symlinks: bool,
    pub error_policy: ErrorPolicy,
    pub child_ordering: ChildOrdering,

    /// If true, entries whose file name begins with '.' are skipped.
    pub skip_hidden: bool,

    /// If true and follow_symlinks == false, symlinks are skipped instead of emitted as File.
    pub skip_symlinks_when_not_following: bool,
}

impl Default for TraverseConfig {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
            error_policy: ErrorPolicy::default(),
            child_ordering: ChildOrdering::Unspecified,
            skip_hidden: false,
            skip_symlinks_when_not_following: false,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameState {
    EmitEnter,
    EmitChildren,
    EmitExit,
}

#[derive(Debug)]
struct DirFrame {
    path: PathBuf,
    raw: OsRawMetadata,
    state: FrameState,

    // buffered children (for future deterministic ordering)
    children: Vec<PathBuf>,
    idx: usize,
}

fn should_halt(reaction: ErrorReaction) -> bool {
    matches!(reaction, ErrorReaction::FailFast)
}

fn order_children(children: &mut Vec<PathBuf>, cfg: &TraverseConfig) {
    match cfg.child_ordering {
        ChildOrdering::Unspecified => {}
        ChildOrdering::PathLexicographic => {
            children.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        }
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn skip_reason(path: &Path, raw: &OsRawMetadata, cfg: &TraverseConfig) -> Option<SkipReason> {
    if cfg.skip_hidden && is_hidden(path) {
        return Some(SkipReason::Hidden);
    }

    if !cfg.follow_symlinks
        && cfg.skip_symlinks_when_not_following
        && raw.kind == FsEntryKind::Symlink
    {
        return Some(SkipReason::Symlink);
    }

    None
}

pub fn traverse(
    root: impl AsRef<Path>,
    cfg: &TraverseConfig,
) -> impl Iterator<Item = TraverseEvent> {
    let root = root.as_ref().to_path_buf();
    let cfg = cfg.clone();

    // DFS stack of directory frames
    let mut stack: Vec<DirFrame> = Vec::new();
    // output buffer so we can emit one event per iterator step cleanly
    let mut out: VecDeque<TraverseEvent> = VecDeque::new();
    let mut halted: bool = false;
    
    // initialization
    match raw_metadata(&root, &cfg) {
        Ok(raw) => {
            if let Some(reason) = skip_reason(&root, &raw, &cfg) {
                out.push_back(TraverseEvent::Skipped {
                    path: root.clone(),
                    reason,
                });
            } else if raw.kind == FsEntryKind::Directory {
                match fs::read_dir(&root) {
                    Ok(rd) => {
                        let mut children = Vec::new();
                        for child in rd {
                            match child {
                                Ok(de) => children.push(de.path()),
                                Err(e) => {
                                    out.push_back(TraverseEvent::Error {
                                        path: root.clone(),
                                        kind: TraverseErrorKind::ReadDirEntryFailed,
                                        message: e.to_string(),
                                    });
                                    if should_halt(cfg.error_policy.on_read_dir_entry_failed) {
                                        halted = true;
                                    }
                                }
                            }
                        }
                        order_children(&mut children, &cfg);

                        // Policy A: only if expansion succeeded do we bracket.
                        stack.push(DirFrame {
                            path: root.clone(),
                            raw: raw.clone(),
                            state: FrameState::EmitEnter,
                            children,
                            idx: 0,
                        });
                    },
                    Err(e) => {
                        out.push_back(TraverseEvent::Error {
                            path: root.clone(),
                            kind: TraverseErrorKind::ExpandFailed,
                            message: e.to_string(),
                        });
                        if should_halt(cfg.error_policy.on_expand_failed) {
                            halted = true;
                        }
                    }
                }
            } else {
                out.push_back(TraverseEvent::File {
                    path: root.clone(),
                    raw,
                });
            }
        },
        Err(msg) => {
            out.push_back(TraverseEvent::Error {
                path: root.clone(),
                kind: TraverseErrorKind::StatFailed,
                message: msg,
            });
            if should_halt(cfg.error_policy.on_stat_failed) {
                halted = true;
            }
        }
    }

    std::iter::from_fn(move || loop {
        if halted {
            return None;
        }

        if let Some(ev) = out.pop_front() {
            return Some(ev);
        }

        let Some(top) = stack.last_mut() else {
            return None;
        };

        match top.state {
            FrameState::EmitEnter => {
                top.state = FrameState::EmitChildren;
                return Some(TraverseEvent::EnterDir {
                    path: top.path.clone(),
                    raw: top.raw.clone(),
                });
            }

            FrameState::EmitChildren => {
                if top.idx >= top.children.len() {
                    top.state = FrameState::EmitExit;
                    continue;
                }

                let child_path = top.children[top.idx].clone();
                top.idx += 1;

                match raw_metadata(&child_path, &cfg) {
                    Ok(raw) => {
                        if let Some(reason) = skip_reason(&child_path, &raw, &cfg) {
                            return Some(TraverseEvent::Skipped {
                                path: child_path,
                                reason,
                            });
                        }

                        if raw.kind == FsEntryKind::Directory {
                            match fs::read_dir(&child_path) {
                                Ok(rd) => {
                                    let mut children = Vec::new();
                                    for child in rd {
                                        match child {
                                            Ok(de) => children.push(de.path()),
                                            Err(e) => {
                                                out.push_back(TraverseEvent::Error {
                                                    path: child_path.clone(),
                                                    kind: TraverseErrorKind::ReadDirEntryFailed,
                                                    message: e.to_string(),
                                                });
                                                if should_halt(cfg.error_policy.on_read_dir_entry_failed) {
                                                    halted = true;
                                                }
                                            }
                                        }
                                    }
                                    order_children(&mut children, &cfg);

                                    // Policy A: only if expansion succeeded do we push a bracketed frame.
                                    stack.push(DirFrame {
                                        path: child_path,
                                        raw,
                                        state: FrameState::EmitEnter,
                                        children,
                                        idx: 0,
                                    });

                                    // loop so next iteration emits the new frame’s EnterDir
                                    continue;
                                }
                                Err(e) => {
                                    let ev = TraverseEvent::Error {
                                        path: child_path,
                                        kind: TraverseErrorKind::ExpandFailed,
                                        message: e.to_string(),
                                    };
                                    if should_halt(cfg.error_policy.on_expand_failed) {
                                        halted = true;
                                    }
                                    return Some(ev);
                                }
                            }
                        } else {
                            return Some(TraverseEvent::File {
                                path: child_path,
                                raw,
                            });
                        }
                    }
                    Err(msg) => {
                        let ev = TraverseEvent::Error {
                            path: child_path,
                            kind: TraverseErrorKind::StatFailed,
                            message: msg,
                        };
                        if should_halt(cfg.error_policy.on_stat_failed) {
                            halted = true;
                        }
                        return Some(ev);
                    }
                }
            }

            FrameState::EmitExit => {
                let finished = stack.pop().expect("frame exists");
                return Some(TraverseEvent::ExitDir { path: finished.path, raw: finished.raw });
            }
        }
    })
}