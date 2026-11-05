//! The committed example session regenerates byte for byte.

mod common;

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    let path = root().join("examples").join(name);
    std::fs::read_to_string(&path).expect("read example")
}

fn example_path(name: &str) -> String {
    root()
        .join("examples")
        .join(name)
        .to_str()
        .expect("utf8")
        .to_string()
}

#[test]
fn the_example_scan_regenerates_byte_for_byte() {
    let dir = common::test_dir("examples");
    let outcome = common::run_cli(&["scan", &example_path("snapshot.txt")], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), example("scan.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_plan_regenerates_byte_for_byte() {
    let dir = common::test_dir("examples");
    let outcome = common::run_cli(&["plan", &example_path("snapshot.txt"), "stale-1"], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), example("plan.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_observation_regenerates_byte_for_byte() {
    let dir = common::test_dir("examples");
    let outcome = common::run_cli(
        &[
            "observe",
            &example_path("snapshot.txt"),
            &example_path("later-snapshot.txt"),
            "stale-1",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), example("observation.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_session_shows_every_classification() {
    let scan = example("scan.txt");
    assert!(scan.contains("counts live 2 suspect 3 proven 2 settled 1"));
    assert!(scan.contains("zombie stale-1"));
    assert!(scan.contains("suspect quiet-1"));
}
