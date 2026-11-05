//! The Round 5 port pins (ZH-25 to ZH-35) that the Python
//! suites could not carry. Dead streams are closed pipes, the
//! only reliable failed write sink on this machine. Boundary
//! carriers are built at the exact sizes the probes measured
//! against the oracle.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use zombie_hunt::judgment::load_fixture_file;
use zombie_hunt::pytext::{PyStr, py_float_repr};

use common::{DeadStream, run_cli, run_cli_dead, test_dir, write, write_bytes};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// The print and error discipline, ZH-28.

fn scan_document_of_size(dir: &Path, target: usize) -> (String, usize) {
    // One proven zombie whose identifier length tunes the
    // scan document one byte per character. Returns the
    // snapshot path and the measured document length.
    let mut filler = 0usize;
    loop {
        let id = format!("z{}", "x".repeat(filler));
        let text = format!(
            "snapshot 100000\nentry {id}\nkind process\nstarter scheduler-1\nstate running\nsignal-interval 120\nlast-signal 96400\nholds cache\nowner done-1\nentry done-1\nkind run\nstarter supervisor-1\nstate finished\nsignal-interval 300\nlast-signal 90000\nholds none\nowner none\n"
        );
        let snap = write(dir, "snap.txt", &text);
        let outcome = run_cli(&["scan", snap.to_str().expect("utf8")], dir);
        assert_eq!(outcome.code, 0);
        let len = outcome.stdout.len();
        if len >= target {
            return (snap.to_str().expect("utf8").to_string(), len);
        }
        filler += target - len;
    }
}

#[test]
fn a_document_under_the_buffer_divide_on_a_dead_stdout_exits_120() {
    let dir = test_dir("port-dead-small");
    let (snap, len) = scan_document_of_size(&dir, 8191);
    assert_eq!(len, 8191);
    let outcome = run_cli_dead(&["scan", &snap], &dir, DeadStream::Stdout);
    assert_eq!(outcome.code, 120);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt could not print its document\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_document_at_the_buffer_divide_on_a_dead_stdout_is_the_uncaught_family() {
    let dir = test_dir("port-dead-big");
    let (snap, len) = scan_document_of_size(&dir, 8192);
    assert_eq!(len, 8192);
    let outcome = run_cli_dead(&["scan", &snap], &dir, DeadStream::Stdout);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: the document print failed\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refusal_on_a_dead_stderr_exits_120() {
    let dir = test_dir("port-dead-refusal");
    let outcome = run_cli_dead(&["scan", "absent.txt"], &dir, DeadStream::Stderr);
    assert_eq!(outcome.code, 120);
    assert_eq!(outcome.stdout_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_usage_error_on_a_dead_stderr_exits_120() {
    let dir = test_dir("port-dead-usage");
    let outcome = run_cli_dead(&[], &dir, DeadStream::Stderr);
    assert_eq!(outcome.code, 120);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_uncaught_line_on_a_dead_stderr_exits_120() {
    let dir = test_dir("port-dead-uncaught");
    let outcome = run_cli_dead(&["observe", "a", "b", "--", "--"], &dir, DeadStream::Stderr);
    // The earlier slot reads first and refuses, and that
    // refusal line cannot be written either, so the run
    // answers 120 with nothing anywhere.
    assert_eq!(outcome.code, 120);
    assert_eq!(outcome.stdout_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_on_a_dead_stdout_exits_120() {
    let dir = test_dir("port-dead-help");
    let outcome = run_cli_dead(&["--help"], &dir, DeadStream::Stdout);
    assert_eq!(outcome.code, 120);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refusal_on_a_dead_stdout_still_answers_two() {
    let dir = test_dir("port-dead-out-refusal");
    let outcome = run_cli_dead(&["scan", "absent.txt"], &dir, DeadStream::Stdout);
    assert_eq!(outcome.code, 2);
    assert!(outcome.stderr_text().starts_with("error: "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_success_on_a_dead_stderr_is_unaffected() {
    let dir = test_dir("port-dead-err-success");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let outcome = run_cli_dead(
        &["scan", snap.to_str().expect("utf8")],
        &dir,
        DeadStream::Stderr,
    );
    assert_eq!(outcome.code, 0);
    assert!(outcome.stdout_text().starts_with("snapshot 100000\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

// The fixture reader ceilings, ZH-30, through the real binary.

fn fixture_dir_with(dir: &Path, name: &str, text: &str) -> String {
    let fixtures = dir.join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixtures dir");
    write(&fixtures, name, text);
    fixtures.to_str().expect("utf8").to_string()
}

fn plan_args(dir: &Path, fixtures: &str) -> (String, String) {
    let snap = write(dir, "snap.txt", common::SNAPSHOT_TEXT);
    (
        snap.to_str().expect("utf8").to_string(),
        fixtures.to_string(),
    )
}

fn run_plan(dir: &Path, snap: &str, fixtures: &str) -> common::Outcome {
    run_cli(&["plan", snap, "stale-1", "--fixtures", fixtures], dir)
}

fn deep_fixture(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

fn mixed_fixture(pairs: usize) -> String {
    format!("{}{}", "{\"a\": [".repeat(pairs), "]}".repeat(pairs))
}

#[test]
fn a_fixture_at_the_nesting_ceiling_loads_and_is_refused_as_a_document() {
    let dir = test_dir("port-ceiling-loads");
    let fixtures = fixture_dir_with(&dir, "deep.json", &deep_fixture(9991));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: fixture deep.json is not an object\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fixture_past_the_nesting_ceiling_is_the_uncaught_family() {
    let dir = test_dir("port-ceiling-over");
    let fixtures = fixture_dir_with(&dir, "deep.json", &deep_fixture(9992));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: the fixture reader exceeded its nesting ceiling\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_mixed_carrier_holds_the_same_ceiling() {
    let dir = test_dir("port-ceiling-mixed-loads");
    let fixtures = fixture_dir_with(&dir, "deep.json", &mixed_fixture(4995));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 2);
    // The mixed carrier's outermost container is an object,
    // so the document refusal names its extra field.
    assert_eq!(
        outcome.stderr_text(),
        "error: fixture deep.json carries the field 'a', which the fixture envelope does not carry\n"
    );
    let dir = test_dir("port-ceiling-mixed-over");
    let fixtures = fixture_dir_with(&dir, "deep.json", &mixed_fixture(4996));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: the fixture reader exceeded its nesting ceiling\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn digit_fixture(digits: usize) -> String {
    format!(
        "{{\"synthetic\": true, \"judgment_point\": \"kill-call\", \
\"request\": {{\"entry\": \"stale-1\", \"signal\": {{\"outcome\": \"satisfied\", \"evidence\": \"x\"}}, \
\"owner\": {{\"outcome\": \"satisfied\", \"evidence\": \"x\"}}, \
\"in-flight\": {{\"outcome\": \"satisfied\", \"evidence\": \"x\"}}}}, \
\"response\": {{\"kind\": \"choice\", \"choice\": \"quarantine\", \
\"confidence\": 1{}, \"probabilities\": {{\"quarantine\": 0.9, \
\"watch\": 0.07, \"spare\": 0.03}}}}}}",
        "0".repeat(digits - 1)
    )
}

#[test]
fn an_integer_at_the_digit_limit_is_read_and_refused_as_a_value() {
    let dir = test_dir("port-digits-at");
    let fixtures = fixture_dir_with(&dir, "digits.json", &digit_fixture(4300));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 2);
    assert!(
        outcome
            .stderr_text()
            .contains("is not a number between 0 and 1"),
        "{}",
        outcome.stderr_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_integer_past_the_digit_limit_is_the_uncaught_family() {
    let dir = test_dir("port-digits-over");
    let fixtures = fixture_dir_with(&dir, "digits.json", &digit_fixture(4301));
    let (snap, fixtures) = plan_args(&dir, &fixtures);
    let outcome = run_plan(&dir, &snap, &fixtures);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: the fixture reader exceeded its integer digit limit\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// The snapshot whole number digit limit, ZH-31, through the
// real binary.

#[test]
fn a_snapshot_time_at_the_digit_limit_converts() {
    let dir = test_dir("port-whole-at");
    let snap = write(
        &dir,
        "snap.txt",
        &format!("snapshot {}\n", "0".repeat(4300)),
    );
    let outcome = run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(
        outcome.stdout_text(),
        "snapshot 0\ncounts live 0 suspect 0 proven 0 settled 0\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_snapshot_time_past_the_digit_limit_is_the_uncaught_family() {
    let dir = test_dir("port-whole-over");
    let text = format!("snapshot {}1\n", "0".repeat(4300));
    let snap = write(&dir, "snap.txt", &text);
    let outcome = run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: a snapshot whole number exceeded its digit limit\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_huge_snapshot_time_prints_in_full_and_ages_exactly() {
    // A 4300 digit time with a last signal one second earlier
    // prints both the time and the exact age in full.
    let dir = test_dir("port-whole-huge");
    let time = format!("1{}", "0".repeat(4299));
    let text = format!(
        "snapshot {time}\nentry a\nkind process\nstarter s\nstate running\nsignal-interval 1\nlast-signal {time}\nholds none\nowner none\n"
    );
    let snap = write(&dir, "snap.txt", &text);
    let outcome = run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 0);
    assert!(
        outcome
            .stdout_text()
            .starts_with(&format!("snapshot {time}\n")),
        "the time prints in full"
    );
    assert!(outcome.stdout_text().contains("counts live 1"));
    let _ = std::fs::remove_dir_all(&dir);
}

// The contracted constants, pinned beside their edges.

#[test]
fn the_contract_constants_carry_their_values() {
    assert_eq!(zombie_hunt::judgment::CONFIDENCE_THRESHOLD, 0.6);
    assert_eq!(zombie_hunt::pyjson::MAX_CONTAINER_DEPTH, 9991);
    assert_eq!(zombie_hunt::pyjson::MAX_INT_DIGITS, 4300);
    assert_eq!(zombie_hunt::snapshot::MAX_WHOLE_DIGITS, 4300);
    assert_eq!(zombie_hunt::scan::PROOF_MULTIPLE, 3);
    assert_eq!(zombie_hunt::plan::OBSERVATION_MULTIPLE, 10);
}

#[test]
fn the_float_printed_form_matches_the_python_repr_rule() {
    assert_eq!(py_float_repr(0.9), "0.9");
    assert_eq!(py_float_repr(0.6), "0.6");
    assert_eq!(py_float_repr(0.75), "0.75");
    assert_eq!(py_float_repr(1.0), "1.0");
    assert_eq!(py_float_repr(0.0), "0.0");
    assert_eq!(py_float_repr(0.0001), "0.0001");
    assert_eq!(py_float_repr(0.00001), "1e-05");
    assert_eq!(py_float_repr(1e16), "1e+16");
    assert_eq!(py_float_repr(1234567890123456.0), "1234567890123456.0");
}

// The reader's own value model, exercised at its public edge.

#[test]
fn a_fixture_file_reads_through_the_library_edge() {
    let dir = test_dir("port-load-edge");
    let path = write(&dir, "one.json", "{}");
    let name = PyStr::from_str("one.json");
    match load_fixture_file(path.to_str().expect("utf8").as_bytes(), &name) {
        Err(error) => {
            assert!(
                error.message().contains("is not marked synthetic"),
                "{}",
                error.message()
            );
        }
        Ok(_) => panic!("a thin fixture must refuse"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// The separator family and list slots, ZH-27.

#[test]
fn a_separator_led_segment_assigns_the_separator_as_a_value() {
    let dir = test_dir("port-sep-led");
    let outcome = run_cli(&["scan", "--", "--"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at -- cannot be read, No such file or directory\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_list_in_the_entry_slot_of_observe_surfaces_at_the_lookup() {
    let dir = test_dir("port-list-entry");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let path = snap.to_str().expect("utf8").to_string();
    let outcome = run_cli(&["observe", &path, &path, "--", "--"], &dir);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: an argument arrived as a list where a single entry identifier was expected\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_list_in_the_later_slot_surfaces_at_that_read() {
    let dir = test_dir("port-list-later");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let path = snap.to_str().expect("utf8").to_string();
    let outcome = run_cli(&["observe", &path, "--", "--", "stale-1"], &dir);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: an argument arrived as a list where a single path was expected\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_earlier_refusal_precedes_a_later_list() {
    let dir = test_dir("port-list-order");
    // The earlier slot holds the literal separator and the
    // entry slot holds a list; the earlier read refusal
    // answers first.
    let outcome = run_cli(&["observe", "--", "--", "stale-1", "--"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at -- cannot be read, No such file or directory\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plan_entry_list_surfaces_after_the_fixtures_load() {
    let dir = test_dir("port-list-plan");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let fixtures = root().join("fixtures");
    let joined = format!("--fixtures={}", fixtures.to_str().expect("utf8"));
    let outcome = run_cli(
        &["plan", &joined, snap.to_str().expect("utf8"), "--", "--"],
        &dir,
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        outcome.stderr_text(),
        "zombie-hunt: an argument arrived as a list where a single entry identifier was expected\n"
    );
    // And a fixtures refusal precedes the same list.
    let outcome = run_cli(
        &[
            "plan",
            "--fixtures=absent-dir",
            snap.to_str().expect("utf8"),
            "--",
            "--",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert!(
        outcome
            .stderr_text()
            .contains("fixtures directory cannot be read, absent-dir"),
        "{}",
        outcome.stderr_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trailing_separator_closing_an_empty_segment_refuses() {
    let dir = test_dir("port-veto");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let fixtures = root().join("fixtures");
    let snap = snap.to_str().expect("utf8");
    let fixtures = fixtures.to_str().expect("utf8");
    // The separator closes the empty segment the consumed
    // flag opened: a usage outcome.
    let outcome = run_cli(
        &["plan", snap, "stale-1", "--fixtures", fixtures, "--"],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    // The same separator after values in the flag's own
    // segment joins their share and is discounted.
    let outcome = run_cli(
        &["plan", "--fixtures", fixtures, snap, "stale-1", "--"],
        &dir,
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), common::ADVANCED_PLAN);
    // No flag consumed, no veto.
    let outcome = run_cli(&["plan", snap, "stale-1", "--"], &dir);
    assert_eq!(outcome.code, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_fires_and_is_blocked_exactly_where_the_oracle_put_it() {
    let dir = test_dir("port-help");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let snap = snap.to_str().expect("utf8");
    for argv in [
        vec!["--help"],
        vec!["--hel"],
        vec!["scan", "--help"],
        vec!["scan", snap, "--help"],
        vec!["scan", "--bogus", "--help"],
        vec!["plan", "--help"],
        vec!["observe", "--help"],
    ] {
        let outcome = run_cli(&argv, &dir);
        assert_eq!(outcome.code, 0, "{argv:?} must print help");
        assert!(outcome.stdout_text().contains("Usage:"), "{argv:?}");
    }
    for argv in [
        vec!["plan", snap, "stale-1", "--fixtures", "--help"],
        vec!["-h=x"],
        vec!["--help=x"],
        vec!["--help="],
        vec!["scan", "--fixtures", "anywhere"],
        vec!["wobble"],
        vec!["scan"],
    ] {
        let outcome = run_cli(&argv, &dir);
        assert_eq!(outcome.code, 2, "{argv:?} must refuse as usage");
        assert_eq!(outcome.stdout_text(), "", "{argv:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_fixtures_option_abbreviates_and_repeats_take_the_last() {
    let dir = test_dir("port-fixtures-forms");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let snap = snap.to_str().expect("utf8");
    let fixtures = root().join("fixtures");
    let fixtures = fixtures.to_str().expect("utf8").to_string();
    for flag in ["--fixtures", "--fixt", "--fi", "--f"] {
        let outcome = run_cli(&["plan", snap, "stale-1", flag, &fixtures], &dir);
        assert_eq!(outcome.code, 0, "{flag} must take the value");
        assert_eq!(outcome.stdout_text(), common::ADVANCED_PLAN);
    }
    let outcome = run_cli(
        &[
            "plan",
            snap,
            "stale-1",
            "--fixtures",
            "absent-dir",
            "--fixtures",
            &fixtures,
        ],
        &dir,
    );
    assert_eq!(outcome.code, 0);
    let outcome = run_cli(
        &[
            "plan",
            snap,
            "stale-1",
            "--fixtures",
            &fixtures,
            "--fixtures",
            "absent-dir",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_fixtures_value_loads_the_current_directory() {
    let dir = test_dir("port-fixtures-empty");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    // A malformed fixture in the working directory proves the
    // empty value loaded it.
    write(&dir, "bad.json", "not json");
    let outcome = run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "stale-1",
            "--fixtures",
            "",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert!(
        outcome
            .stderr_text()
            .contains("fixture file is not valid JSON, bad.json"),
        "{}",
        outcome.stderr_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_negative_looking_value_is_a_value() {
    let dir = test_dir("port-negative");
    let outcome = run_cli(&["scan", "-1"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at -1 cannot be read, No such file or directory\n"
    );
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let path = snap.to_str().expect("utf8");
    let outcome = run_cli(&["observe", path, path, "-1"], &dir);
    assert_eq!(outcome.code, 2);
    assert!(
        outcome.stderr_text().contains("no entry in"),
        "{}",
        outcome.stderr_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// Paths, probed at the open, ZH-29.

#[test]
fn the_cannot_read_line_names_the_path_as_written() {
    let dir = test_dir("port-path-names");
    for spelling in ["ghost.txt/", "ghost.txt/.", "./ghost.txt", "a//ghost.txt"] {
        let outcome = run_cli(&["scan", spelling], &dir);
        assert_eq!(outcome.code, 2, "{spelling}");
        assert_eq!(
            outcome.stderr_text(),
            format!(
                "error: the snapshot at {spelling} cannot be read, No such file or directory\n"
            ),
            "{spelling}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_spelled_with_a_trailing_slash_or_dot_reads_as_the_file() {
    let dir = test_dir("port-path-folds");
    write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    std::fs::create_dir_all(dir.join("sub")).expect("sub dir");
    for spelling in [
        "snap.txt/",
        "snap.txt/.",
        "./snap.txt",
        "snap.txt//",
        "sub/../snap.txt",
        ".//snap.txt",
    ] {
        let outcome = run_cli(&["scan", spelling], &dir);
        assert_eq!(outcome.code, 0, "{spelling} must read the file");
        assert_eq!(outcome.stdout_text(), common::EXPECTED_SCAN);
    }
    let outcome = run_cli(&["scan", ""], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at  cannot be read, Is a directory\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fixture_child_directory_refuses_with_the_folded_joined_path() {
    let dir = test_dir("port-path-fixture");
    let snap = write(&dir, "snap.txt", common::SNAPSHOT_TEXT);
    let fixtures = dir.join("fx");
    std::fs::create_dir_all(fixtures.join("inner.json")).expect("child dir");
    for spelling in ["fx", "fx/", "fx/.", "./fx", "fx//"] {
        let outcome = run_cli(
            &[
                "plan",
                snap.to_str().expect("utf8"),
                "stale-1",
                "--fixtures",
                spelling,
            ],
            &dir,
        );
        assert_eq!(outcome.code, 2, "{spelling}");
        assert_eq!(
            outcome.stderr_text(),
            "error: fixture file cannot be read, fx/inner.json\n",
            "{spelling}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_utf8_refusal_names_the_path_as_written_under_a_folded_spelling() {
    let dir = test_dir("port-path-utf8");
    write_bytes(&dir, "snap.txt", b"snapshot 100000\nentry a\xff\n");
    let outcome = run_cli(&["scan", "./snap.txt"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at ./snap.txt is not readable as UTF-8 text\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_parse_refusal_names_the_path_as_written() {
    let dir = test_dir("port-path-parse");
    write(&dir, "snap.txt", "snapshot soon\n");
    let outcome = run_cli(&["scan", "./snap.txt"], &dir);
    assert_eq!(outcome.code, 2);
    assert!(
        outcome.stderr_text().contains("./snap.txt is malformed"),
        "{}",
        outcome.stderr_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_non_utf8_file_name_displays_with_surrogate_escapes() {
    let dir = test_dir("port-path-surrogate");
    let outcome = common::run_cli_bytes(&[b"scan", b"ghost\xff.txt"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(
        outcome.stderr_text(),
        "error: the snapshot at ghost\\udcff.txt cannot be read, No such file or directory\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// The manifest and the shipped sources, ZH-34.

fn manifest_sections(text: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(done) = current.take() {
                sections.push(done);
            }
            current = Some((trimmed.trim_matches(['[', ']']).to_string(), String::new()));
        } else if let Some((_, body)) = &mut current {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(done) = current.take() {
        sections.push(done);
    }
    sections
}

fn dependency_header(header: &str) -> Option<(&'static str, Option<String>)> {
    const KINDS: [&str; 3] = ["dev-dependencies", "build-dependencies", "dependencies"];
    let header = header.trim();
    for kind in KINDS {
        if header == kind {
            return Some((kind, None));
        }
        if let Some(rest) = header.strip_suffix(&format!(".{kind}")) {
            let _ = rest;
            return Some((kind, None));
        }
        if let Some(rest) = header.strip_prefix(&format!("{kind}.")) {
            return Some((kind, Some(rest.trim_matches(['\'', '"']).to_string())));
        }
    }
    None
}

fn body_crate_names(body: &str, out: &mut BTreeSet<String>) {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            let name = name.trim().trim_matches(['"', '\'']);
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                out.insert(name.to_string());
            }
        }
    }
}

fn dependency_sets(text: &str) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut shipped = BTreeSet::new();
    let mut dev = BTreeSet::new();
    let mut build = BTreeSet::new();
    for (header, body) in manifest_sections(text) {
        let Some((kind, dotted)) = dependency_header(&header) else {
            continue;
        };
        let target = match kind {
            "dev-dependencies" => &mut dev,
            "build-dependencies" => &mut build,
            _ => &mut shipped,
        };
        if let Some(name) = dotted {
            target.insert(name);
        }
        body_crate_names(&body, target);
    }
    (shipped, dev, build)
}

#[test]
fn the_manifest_places_its_dependencies_as_the_contract_states() {
    let text = std::fs::read_to_string(root().join("Cargo.toml")).expect("read manifest");
    let (shipped, dev, _build) = dependency_sets(&text);
    for crate_name in ["anyhow", "clap", "thiserror"] {
        assert!(
            shipped.contains(crate_name),
            "{crate_name} is a shipped dependency"
        );
    }
    for crate_name in ["serde", "serde_json", "sha2"] {
        assert!(
            !shipped.contains(crate_name),
            "{crate_name} may not be a shipped dependency"
        );
        assert!(
            dev.contains(crate_name) || crate_name == "serde",
            "{crate_name} stands in dev dependencies where it stands at all"
        );
    }
    assert!(
        dev.contains("serde_json"),
        "serde_json is a dev dependency only"
    );
    assert!(dev.contains("sha2"), "sha2 is a dev dependency only");
    // The reader itself, over the manifest forms the contract
    // names, so the pin cannot pass by reading nothing.
    let (shipped, _, _) = dependency_sets("[dependencies.serde]\nversion = \"1\"\n");
    assert!(shipped.contains("serde"), "dotted table form must read");
    let (shipped, _, _) = dependency_sets("[target.'cfg(unix)'.dependencies]\nserde = \"1\"\n");
    assert!(shipped.contains("serde"), "target dotted form must read");
    let (_, _, build) = dependency_sets("[build-dependencies]\nserde_json = \"1\"\n");
    assert!(build.contains("serde_json"), "build dependencies must read");
    let (_, dev, _) = dependency_sets("[dev-dependencies.serde]\nversion = \"1\"\n");
    assert!(dev.contains("serde"), "dotted dev form must read");
}

#[test]
fn no_python_files_stand_in_the_tree() {
    let root = root();
    let mut bad = Vec::new();
    fn walk(dir: &Path, root: &Path, bad: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().expect("type").is_dir() {
                if name == ".git" || name == "target" {
                    continue;
                }
                walk(&path, root, bad);
            } else if name.ends_with(".py") || name == "pyproject.toml" || name == "conftest.py" {
                bad.push(
                    path.strip_prefix(root)
                        .expect("under root")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    walk(&root, &root, &mut bad);
    assert!(bad.is_empty(), "python files remain: {bad:?}");
}

/// The crate roots one source text imports, resolving use
/// trees in grouped, nested, aliased, and glob forms, and
/// bare module imports.
fn imported_roots(text: &str) -> Vec<String> {
    let mut roots = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("use ") {
            let first = rest
                .split([':', ';', '{', ' '])
                .next()
                .expect("a root stands");
            if !first.is_empty() {
                roots.push(first.to_string());
            }
        }
    }
    roots
}

#[test]
fn the_import_scan_resolves_every_use_form() {
    assert_eq!(imported_roots("use serde_json::Value;\n"), ["serde_json"]);
    assert_eq!(
        imported_roots("use serde::{Serialize, de::DeserializeOwned};\n"),
        ["serde"]
    );
    assert_eq!(
        imported_roots("use serde::de::{self, DeserializeOwned};\n"),
        ["serde"]
    );
    assert_eq!(imported_roots("use sha2::Digest as _;\n"), ["sha2"]);
    assert_eq!(imported_roots("use clap::*;\n"), ["clap"]);
    assert_eq!(imported_roots("use serde;\n"), ["serde"]);
}

#[test]
fn the_shipped_sources_never_import_the_serde_family() {
    let src = root().join("src");
    for entry in std::fs::read_dir(&src).expect("read src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read source");
        for imported in imported_roots(&text) {
            assert!(
                imported != "serde" && imported != "serde_json",
                "{} imports {imported}",
                path.display()
            );
        }
    }
}

#[test]
fn anyhow_is_confined_to_the_binary_entry() {
    let src = root().join("src");
    for entry in std::fs::read_dir(&src).expect("read src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read source");
        if path.file_name().and_then(|n| n.to_str()) == Some("main.rs") {
            continue;
        }
        for imported in imported_roots(&text) {
            assert_ne!(
                imported,
                "anyhow",
                "{} imports anyhow outside the binary entry",
                path.display()
            );
        }
    }
}
