//! ZH-11 and ZH-12. The command line tool and its discipline.

mod common;

use common::{EXPECTED_SCAN, SNAPSHOT_TEXT};

#[test]
fn scan_prints_the_document_and_exits_zero() {
    let dir = common::test_dir("cli-scan");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let outcome = common::run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), EXPECTED_SCAN);
    assert_eq!(outcome.stderr_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plan_prints_the_document_and_exits_zero() {
    let dir = common::test_dir("cli-plan");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let outcome = common::run_cli(&["plan", snap.to_str().expect("utf8"), "stale-1"], &dir);
    assert_eq!(outcome.code, 0);
    assert!(
        outcome
            .stdout_text()
            .starts_with("quarantine plan for stale-1\n")
    );
    assert_eq!(outcome.stderr_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_malformed_snapshot_exits_two_with_one_error_line() {
    let dir = common::test_dir("cli-malformed");
    let snap = common::write(&dir, "snap.txt", "snapshot 100000\nentry a\nkind daemon\n");
    let outcome = common::run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(!outcome.stderr_text().contains("Traceback"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plan_refusal_exits_two_with_one_error_line() {
    let dir = common::test_dir("cli-plan-refusal");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let outcome = common::run_cli(&["plan", snap.to_str().expect("utf8"), "busy-1"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(outcome.stderr_text().contains("classified suspect"));
    assert!(!outcome.stderr_text().contains("Traceback"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_exits_two_with_one_error_line() {
    let dir = common::test_dir("cli-missing");
    let absent = dir.join("absent.txt");
    let outcome = common::run_cli(&["scan", absent.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_non_utf8_snapshot_exits_two_with_one_error_line() {
    let dir = common::test_dir("cli-non-utf8");
    let snap = common::write_bytes(&dir, "snap.txt", b"snapshot 100000\nentry a\xff\n");
    let outcome = common::run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_commands_modify_none_of_their_inputs() {
    let dir = common::test_dir("cli-inputs");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let before = common::read(&snap);
    assert_eq!(
        common::run_cli(&["scan", snap.to_str().expect("utf8")], &dir).code,
        0
    );
    assert_eq!(
        common::run_cli(&["plan", snap.to_str().expect("utf8"), "stale-1"], &dir).code,
        0
    );
    assert_eq!(common::read(&snap), before);
    let _ = std::fs::remove_dir_all(&dir);
}
