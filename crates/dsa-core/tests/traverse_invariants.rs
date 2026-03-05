use dsa_core::traverse::TraverseConfig;
use dsa_core::traverse::traverse;
use dsa_core::events::{TraverseEvent, TraverseErrorKind};
use dsa_core::policy::{ErrorPolicy, ErrorReaction};

use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
fn make_tmp_base(name: &str) -> PathBuf {
    ["target", "tmp_tests", name].iter().collect()
}

#[cfg(unix)]
fn cleanup_dir(path: &PathBuf) {
    let _ = fs::remove_dir_all(path);
}

#[cfg(unix)]
fn collect_events(base: &PathBuf, cfg: &TraverseConfig) -> Vec<TraverseEvent> {
    traverse(base, cfg).collect()
}

#[cfg(unix)]
fn mk_file(path: &PathBuf, bytes: &[u8]) {
    fs::write(path, bytes).expect("write file");
}

#[cfg(unix)]
fn mk_dir(path: &PathBuf) {
    fs::create_dir_all(path).expect("create dir");
}

#[cfg(unix)]
#[test]
fn policy_a_unreadable_dir_emits_expand_failed_error_but_no_brackets_for_that_dir() {
    use std::os::unix::fs::PermissionsExt;

    // Arrange
    let base = make_tmp_base("policy_a_unreadable_dir_no_brackets");
    let dir = base.join("no_read");

    cleanup_dir(&base);
    mk_dir(&dir);

    // Make directory unreadable so read_dir fails.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).expect("chmod 000");

    // Act
    let cfg = TraverseConfig::default();
    let events = collect_events(&base, &cfg);

    // Cleanup (restore perms so removal works)
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).ok();
    cleanup_dir(&base);

    // Assert: Exactly one ExpandFailed error for `dir`
    let expand_failed_count = events.iter().filter(|e| {
        matches!(e, TraverseEvent::Error { path, kind: TraverseErrorKind::ExpandFailed, .. } if *path == dir)
    }).count();
    assert_eq!(expand_failed_count, 1, "expected exactly one ExpandFailed error for unreadable dir");

    // Assert: No EnterDir/ExitDir for that directory (Policy A)
    let saw_enter = events.iter().any(|e| matches!(e, TraverseEvent::EnterDir { path, .. } if *path == dir));
    let saw_exit  = events.iter().any(|e| matches!(e, TraverseEvent::ExitDir { path, .. } if *path == dir));
    assert!(!saw_enter, "should not emit EnterDir for unreadable directory under Policy A");
    assert!(!saw_exit,  "should not emit ExitDir for unreadable directory under Policy A");
}

#[cfg(unix)]
#[test]
fn well_parenthesized_enter_exit_never_underflows_and_finishes_empty() {
    // Arrange: a simple tree
    let base = make_tmp_base("well_parenthesized");
    cleanup_dir(&base);
    mk_dir(&base);
    mk_dir(&base.join("a"));
    mk_dir(&base.join("a").join("b"));
    mk_file(&base.join("a").join("b").join("f.txt"), b"hi");
    mk_file(&base.join("root.txt"), b"root");

    // Act
    let cfg = TraverseConfig::default();
    let events = collect_events(&base, &cfg);

    cleanup_dir(&base);

    // Assert: bracket stack discipline
    let mut stack: Vec<PathBuf> = Vec::new();

    for ev in events {
        match ev {
            TraverseEvent::EnterDir { path, .. } => stack.push(path),
            TraverseEvent::ExitDir { path, .. } => {
                let Some(open) = stack.pop() else {
                    panic!("ExitDir({path:?}) with empty stack (underflow)");
                };
                assert_eq!(open, path, "ExitDir did not match most recent EnterDir (nesting violated)");
            }
            _ => {}
        }
    }

    assert!(stack.is_empty(), "stack not empty at end; missing ExitDir events");
}

#[cfg(unix)]
#[test]
fn enter_dir_and_exit_dir_emitted_for_readable_directories() {
    // Arrange
    let base = make_tmp_base("enter_exit_for_readable_dirs");
    cleanup_dir(&base);
    mk_dir(&base);
    mk_dir(&base.join("d1"));
    mk_dir(&base.join("d1").join("d2"));
    mk_file(&base.join("d1").join("d2").join("x.bin"), b"123");

    // Act
    let cfg = TraverseConfig::default();
    let events = collect_events(&base, &cfg);

    cleanup_dir(&base);

    // Assert: for each readable dir we created, we should see Enter + Exit.
    for dir in [&base, &base.join("d1"), &base.join("d1").join("d2")] {
        let enter_count = events.iter().filter(|e| matches!(e, TraverseEvent::EnterDir { path, .. } if path == dir)).count();
        let exit_count  = events.iter().filter(|e| matches!(e, TraverseEvent::ExitDir { path, .. } if path == dir)).count();
        assert_eq!(enter_count, 1, "expected exactly one EnterDir for {dir:?}");
        assert_eq!(exit_count, 1, "expected exactly one ExitDir for {dir:?}");
    }
}

#[cfg(unix)]
#[test]
fn default_policy_is_non_fatal_traversal_continues_after_error() {
    use std::os::unix::fs::PermissionsExt;

    // Arrange
    let base = make_tmp_base("default_non_fatal_continues");
    cleanup_dir(&base);
    mk_dir(&base);
    mk_file(&base.join("ok1.txt"), b"ok");

    let bad_dir = base.join("no_read");
    mk_dir(&bad_dir);
    fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o000)).expect("chmod 000");

    mk_file(&base.join("ok2.txt"), b"ok");

    // Act
    let cfg = TraverseConfig::default();
    let events = collect_events(&base, &cfg);

    // Cleanup
    fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o700)).ok();
    cleanup_dir(&base);

    // Assert: saw the error
    let first_error_idx = events.iter().position(|e| {
        matches!(e, TraverseEvent::Error { path, kind: TraverseErrorKind::ExpandFailed, .. } if *path == bad_dir)
    }).expect("expected ExpandFailed error for unreadable dir");

    // Assert: traversal continued: some event exists after that error (ideally File ok2)
    assert!(events.len() > first_error_idx + 1, "expected traversal to continue after error under default policy");
}

#[cfg(unix)]
#[test]
fn fail_fast_stops_after_first_error_event() {
    use std::os::unix::fs::PermissionsExt;

    // Arrange
    let base = make_tmp_base("fail_fast_stops");
    cleanup_dir(&base);
    mk_dir(&base);
    mk_file(&base.join("ok1.txt"), b"ok");

    let bad_dir = base.join("no_read");
    mk_dir(&bad_dir);
    fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o000)).expect("chmod 000");

    mk_file(&base.join("ok2.txt"), b"ok");

    // Act
    let mut cfg = TraverseConfig::default();
    cfg.error_policy = ErrorPolicy {
        on_stat_failed: ErrorReaction::FailFast,
        on_expand_failed: ErrorReaction::FailFast,
        on_read_dir_entry_failed: ErrorReaction::FailFast,
    };

    let events = collect_events(&base, &cfg);

    // Cleanup
    fs::set_permissions(&bad_dir, fs::Permissions::from_mode(0o700)).ok();
    cleanup_dir(&base);

    // Assert: last event is an Error, and we did not continue past it
    assert!(!events.is_empty());
    assert!(matches!(events.last().unwrap(), TraverseEvent::Error { .. }), "expected traversal to stop on an Error in fail-fast mode");
}