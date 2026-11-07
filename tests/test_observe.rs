//! ZH-21 to ZH-24. The observation report over the plan's watch.

mod common;

use odradek::errors::Error;
use odradek::observe::build_observation;
use odradek::pytext::PyStr;
use odradek::scan::{classify, classify_entry};
use odradek::snapshot::parse_snapshot;

use common::EARLIER_TEXT;

const CLOSING: &str =
    "executes nothing, this report is a document and the tool performs no part of it\n";

fn observe(
    earlier_text: &str,
    later_text: &str,
    entry_id: &str,
) -> odradek::errors::Result<String> {
    let earlier = parse_snapshot(
        &PyStr::from_str(earlier_text),
        &PyStr::from_str("earlier.txt"),
    )
    .expect("earlier parses");
    let later = parse_snapshot(&PyStr::from_str(later_text), &PyStr::from_str("later.txt"))
        .expect("later parses");
    let verdicts = classify(&earlier).expect("classification stands");
    build_observation(&earlier, &later, &verdicts, &PyStr::from_str(entry_id))
}

fn later_at(time: u64, text: &str) -> String {
    text.replacen("snapshot 100000", &format!("snapshot {time}"), 1)
}

fn without_stale_1(text: &str) -> String {
    let start = text.find("entry stale-1").expect("stale-1 block");
    let end = text.find("entry done-run").expect("done-run block");
    format!("{}{}", &text[..start], &text[end..])
}

#[test]
fn the_watch_is_still_running_before_the_window() {
    let document =
        observe(EARLIER_TEXT, &later_at(100500, EARLIER_TEXT), "stale-1").expect("observation");
    assert_eq!(
        document,
        format!(
            "observation report for stale-1\nplanned from snapshot 100000\nobserved at snapshot 100500\nelapsed 500 seconds\nobservation window 1200 seconds\noutcome watch still running\n{CLOSING}"
        )
    );
}

#[test]
fn the_quarantine_held_at_the_exact_window_boundary() {
    let document =
        observe(EARLIER_TEXT, &later_at(101200, EARLIER_TEXT), "stale-1").expect("observation");
    assert_eq!(
        document,
        format!(
            "observation report for stale-1\nplanned from snapshot 100000\nobserved at snapshot 101200\nelapsed 1200 seconds\nobservation window 1200 seconds\noutcome quarantine held\n  evidence for the later snapshot records last signal 96400 for the entry, the same last signal the earlier snapshot recorded\n  evidence for owner done-run is recorded finished\n{CLOSING}"
        )
    );
}

#[test]
fn the_quarantine_broke_on_a_later_signal_alone() {
    let later = later_at(101500, EARLIER_TEXT).replace("last-signal 96400", "last-signal 100800");
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine broken\n"));
    assert!(document.contains(
        "  evidence against the later snapshot records last signal 100800 for the entry, later than the last signal 96400 the earlier snapshot recorded\n"
    ));
    assert!(document.contains("  evidence for owner done-run is recorded finished\n"));
}

#[test]
fn the_quarantine_broke_on_an_owner_recorded_running() {
    let later = later_at(101500, EARLIER_TEXT).replace("owner done-run", "owner boss-run");
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine broken\n"));
    assert!(document.contains("  evidence against owner boss-run is recorded running\n"));
    assert!(document.contains(
        "  evidence for the later snapshot records last signal 96400 for the entry, the same last signal the earlier snapshot recorded\n"
    ));
}

#[test]
fn the_quarantine_held_when_the_entry_is_absent_later() {
    let later = later_at(101500, &without_stale_1(EARLIER_TEXT));
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine held\n"));
    assert!(document.contains(
        "  evidence for the entry is absent from the later snapshot, so no signal from it and no owner for it is recorded there\n"
    ));
}

#[test]
fn the_window_comes_from_the_earlier_interval_alone() {
    let later = later_at(100500, EARLIER_TEXT).replace("signal-interval 120", "signal-interval 10");
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("observation window 1200 seconds\n"));
    assert!(document.contains("outcome watch still running\n"));
}

#[test]
fn a_settled_later_entry_that_signalled_broke_the_watch() {
    let later = later_at(101500, EARLIER_TEXT)
        .replace("last-signal 96400", "last-signal 100800")
        .replace(
            "state running\nsignal-interval 120",
            "state finished\nsignal-interval 120",
        );
    let later_snapshot = parse_snapshot(&PyStr::from_str(&later), &PyStr::from_str("later.txt"))
        .expect("later parses");
    let entry = later_snapshot
        .entry(&PyStr::from_str("stale-1"))
        .expect("entry stands");
    assert_eq!(
        classify_entry(&later_snapshot, entry)
            .expect("classification stands")
            .classification,
        "settled"
    );
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine broken\n"));
}

#[test]
fn an_earlier_signal_value_later_is_no_sign_of_life() {
    let later = later_at(101500, EARLIER_TEXT).replace("last-signal 96400", "last-signal 90000");
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine held\n"));
    assert!(document.contains(
        "  evidence for the later snapshot records last signal 90000 for the entry, which is not later than the last signal 96400 the earlier snapshot recorded\n"
    ));
}

#[test]
fn no_signal_recorded_later_is_no_sign_of_life() {
    let later = later_at(101500, EARLIER_TEXT).replace("last-signal 96400", "last-signal none");
    let document = observe(EARLIER_TEXT, &later, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine held\n"));
    assert!(
        document.contains("  evidence for the later snapshot records no signal for the entry\n")
    );
}

#[test]
fn an_absent_or_unrecorded_owner_later_is_no_sign_of_life() {
    let absent = later_at(101500, EARLIER_TEXT).replace("owner done-run", "owner ghost-7");
    let document = observe(EARLIER_TEXT, &absent, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine held\n"));
    assert!(document.contains("  evidence for owner ghost-7 is absent from the snapshot\n"));
    let unrecorded = later_at(101500, EARLIER_TEXT).replace("owner done-run", "owner none");
    let document = observe(EARLIER_TEXT, &unrecorded, "stale-1").expect("observation");
    assert!(document.contains("outcome quarantine held\n"));
    assert!(document.contains("  evidence for no owner reference is recorded for this entry\n"));
}

#[test]
fn the_report_is_a_pure_function_of_the_two_texts() {
    let later = later_at(101500, EARLIER_TEXT);
    assert_eq!(
        observe(EARLIER_TEXT, &later, "stale-1").expect("first"),
        observe(EARLIER_TEXT, &later, "stale-1").expect("second")
    );
}

#[test]
fn a_later_snapshot_not_later_is_refused() {
    match observe(EARLIER_TEXT, EARLIER_TEXT, "stale-1") {
        Err(Error::Observe(message)) => {
            assert!(message.contains("stale-1"));
            assert!(message.contains("100000"));
            assert!(message.contains("not later"));
        }
        other => panic!("expected an observe refusal, got {other:?}"),
    }
    assert!(matches!(
        observe(EARLIER_TEXT, &later_at(99999, EARLIER_TEXT), "stale-1"),
        Err(Error::Observe(_))
    ));
}

#[test]
fn an_identifier_the_earlier_snapshot_lacks_is_refused() {
    match observe(EARLIER_TEXT, &later_at(101500, EARLIER_TEXT), "ghost-9") {
        Err(Error::Observe(message)) => {
            assert!(message.contains("ghost-9"));
            assert!(message.contains("no entry in earlier.txt"));
        }
        other => panic!("expected an observe refusal, got {other:?}"),
    }
}

#[test]
fn an_entry_not_proven_in_the_earlier_snapshot_is_refused() {
    match observe(EARLIER_TEXT, &later_at(101500, EARLIER_TEXT), "busy-1") {
        Err(Error::Observe(message)) => {
            assert!(message.contains("classified suspect"), "{message:?}");
        }
        other => panic!("expected an observe refusal, got {other:?}"),
    }
    match observe(EARLIER_TEXT, &later_at(101500, EARLIER_TEXT), "done-run") {
        Err(Error::Observe(message)) => {
            assert!(message.contains("classified settled"), "{message:?}");
        }
        other => panic!("expected an observe refusal, got {other:?}"),
    }
}

#[test]
fn observe_prints_the_document_and_exits_zero() {
    let dir = common::test_dir("observe-cli");
    let earlier = common::write(&dir, "early.txt", EARLIER_TEXT);
    let later = common::write(&dir, "later.txt", &later_at(101200, EARLIER_TEXT));
    let outcome = common::run_cli(
        &[
            "observe",
            earlier.to_str().expect("utf8"),
            later.to_str().expect("utf8"),
            "stale-1",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(
        outcome.stdout_text(),
        observe(EARLIER_TEXT, &later_at(101200, EARLIER_TEXT), "stale-1").expect("observation")
    );
    assert_eq!(outcome.stderr_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_observe_refusal_exits_two_with_one_error_line() {
    let dir = common::test_dir("observe-cli-refusal");
    let earlier = common::write(&dir, "early.txt", EARLIER_TEXT);
    let later = common::write(&dir, "later.txt", &later_at(101500, EARLIER_TEXT));
    let outcome = common::run_cli(
        &[
            "observe",
            earlier.to_str().expect("utf8"),
            later.to_str().expect("utf8"),
            "busy-1",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(outcome.stderr_text().contains("classified suspect"));
    assert!(!outcome.stderr_text().contains("Traceback"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn observe_accepts_no_fixtures_option() {
    let dir = common::test_dir("observe-cli-fixtures");
    let earlier = common::write(&dir, "early.txt", EARLIER_TEXT);
    let later = common::write(&dir, "later.txt", &later_at(101500, EARLIER_TEXT));
    let outcome = common::run_cli(
        &[
            "observe",
            earlier.to_str().expect("utf8"),
            later.to_str().expect("utf8"),
            "stale-1",
            "--fixtures",
            "anywhere",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn observe_modifies_none_of_its_inputs() {
    let dir = common::test_dir("observe-cli-inputs");
    let earlier = common::write(&dir, "early.txt", EARLIER_TEXT);
    let later = common::write(&dir, "later.txt", &later_at(101200, EARLIER_TEXT));
    let before = (common::read(&earlier), common::read(&later));
    let outcome = common::run_cli(
        &[
            "observe",
            earlier.to_str().expect("utf8"),
            later.to_str().expect("utf8"),
            "stale-1",
        ],
        &dir,
    );
    assert_eq!(outcome.code, 0);
    assert_eq!((common::read(&earlier), common::read(&later)), before);
    let _ = std::fs::remove_dir_all(&dir);
}
