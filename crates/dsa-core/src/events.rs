use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// Best-effort metadata obtained directly from the operating system.
#[derive(Debug, Clone)]
pub struct OsRawMetadata {
    pub kind: FsEntryKind,
    pub modified: Option<SystemTime>,

    /// Logical file length in bytes if available (for regular files).
    /// For non-files, this may be None.
    pub logical_len_bytes: Option<u64>,
}

/// Metadata computed by the engine from the event stream.
/// It May depend on aggregation / policy decisions.
#[derive(Debug, Clone, Default)]
pub struct DerivedMetadata {
    /// Example derived field we will compute later: ("size on disk" of directories even though technically they are zero bytes).
    /// total size of subtree rooted at this entry (files only / policy-defined).
    pub subtree_logical_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraverseErrorKind {
    StatFailed,
    ExpandFailed,
    ReadDirEntryFailed,
}

#[derive(Debug, Clone)]
pub enum TraverseEvent {
    Entry {
        path: PathBuf,
        raw: OsRawMetadata,
    },
    Error {
        path: PathBuf,
        kind: TraverseErrorKind,
        message: String,
    },
}
