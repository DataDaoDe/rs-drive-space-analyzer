use dsa_core::events::{TraverseEvent, TraverseErrorKind};
use dsa_core::traverse::{traverse, TraverseConfig};

use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
#[test]
fn emits_entry_even_if_directory_cannot_be_read() {
    use std::os::unix::fs::PermissionsExt;

    // Arrange: create temp directory structure inside target (safe enough for now)
    // We'll use a unique-ish folder name.
    let base: PathBuf = ["target", "tmp_test_no_read"].iter().collect();
    let dir = base.join("no_read");

    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&dir).expect("create test dir");

    // Remove read/execute permissions from `no_read` so read_dir fails.
    // (0o000 means no permissions)
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000))
        .expect("chmod 000");

    // Act
    let cfg = TraverseConfig::default();
    let events: Vec<TraverseEvent> = traverse(&base, &cfg).collect();

    // Cleanup: restore permissions so we can delete it
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .ok();
    let _ = fs::remove_dir_all(&base);

    // Assert: we must see an Entry for `no_read` (stat is allowed),
    // and we should see at least one Error about expanding it.
    let mut saw_entry = false;
    let mut saw_error = false;

    for ev in events {
        match ev {
            TraverseEvent::Entry { path, .. } => {
                if path == dir {
                    saw_entry = true;
                }
            }
            TraverseEvent::Error { path, kind, .. } => {
                if path == dir && kind == TraverseErrorKind::ExpandFailed {
                    saw_error = true;
                }
            }
        }
    }

    assert!(saw_entry, "Expected Entry event for unreadable directory");
    assert!(saw_error, "Expected Error event when expanding unreadable directory");
}