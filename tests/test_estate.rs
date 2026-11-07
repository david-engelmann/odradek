//! ZH-36 to ZH-42. The estate plan over every proven zombie.

mod common;

use std::cell::RefCell;
use std::path::PathBuf;

use odradek::errors::Error;
use odradek::judgment::{FixturePlayer, JudgmentSource, kill_call_request};
use odradek::plan::{build_estate_plan, build_plan};
use odradek::pyjson::{Value, dumps_canonical, loads};
use odradek::pytext::PyStr;
use odradek::scan::classify;
use odradek::snapshot::parse_snapshot;

use common::{ADVANCED_PLAN, EXPECTED_PLAN, SNAPSHOT_B, SNAPSHOT_TEXT};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn v(text: &str) -> Value {
    loads(&PyStr::from_str(text)).expect("json parses")
}

fn canonical(value: &Value) -> String {
    let mut out = String::new();
    dumps_canonical(value, &mut out);
    out
}

/// The single plan document for stale-2 over SNAPSHOT_TEXT,
/// written out in full so the estate expectation below is a
/// literal and not a call into the builder it pins.
const STALE_2_PLAN: &str = "quarantine plan for stale-2\nclassification proven zombie\nproof\n  evidence for signal age 1000 seconds reaches the proof threshold of 180 seconds\n  evidence for owner ghost-7 is absent from the snapshot\n  evidence for the entry holds nothing and carries no in-flight marker\nstep 1 reversible, detach stale-2 from cron-1 so no new work reaches it\nstep 2 reversible, record that the entry holds nothing, so no resource needs fencing while the entry is watched\nstep 3 reversible, hold the quarantine for the observation window and watch for any signal from stale-2\nobservation window 600 seconds\nescalation, if stale-2 shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself\nexecutes nothing, this plan is a document and the tool performs no part of it\n";

const CLOSING: &str =
    "executes nothing, this plan is a document and the tool performs no part of it\n";

fn estate(text: &str, source: Option<&dyn JudgmentSource>) -> Result<String, Error> {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    build_estate_plan(
        &snap,
        &classify(&snap).expect("classification stands"),
        source,
    )
}

/// A judgment source that records every request it is asked
/// and answers from a per entry table, abstaining where the
/// table carries no answer for the entry.
struct TableSource {
    answers: Vec<(String, Option<Value>)>,
    requests: RefCell<Vec<Value>>,
}

impl TableSource {
    fn new(answers: Vec<(String, Option<Value>)>) -> TableSource {
        TableSource {
            answers,
            requests: RefCell::new(Vec::new()),
        }
    }
}

impl JudgmentSource for TableSource {
    fn judge(&self, request: &Value) -> Option<Value> {
        self.requests.borrow_mut().push(request.clone());
        let entry = request
            .get("entry")
            .and_then(|held| held.as_str())
            .expect("a request names its entry")
            .to_display();
        self.answers
            .iter()
            .find(|(name, _)| name == &entry)
            .and_then(|(_, answer)| answer.clone())
    }
}

const QUARANTINE_JSON: &str = "{\"kind\": \"choice\", \"choice\": \"quarantine\", \"confidence\": 0.9, \"probabilities\": {\"quarantine\": 0.9, \"watch\": 0.07, \"spare\": 0.03}}";
const ABSTENTION_JSON: &str = "{\"kind\": \"abstention\"}";

// ZH-37. The estate document, its framing, and its order.

#[test]
fn the_estate_over_the_example_estate_is_the_framed_set_of_single_plans() {
    let expected = format!("estate quarantine plan\n{EXPECTED_PLAN}{STALE_2_PLAN}{CLOSING}");
    assert_eq!(estate(SNAPSHOT_TEXT, None).expect("estate"), expected);
}

#[test]
fn every_estate_block_is_byte_for_byte_its_single_plan() {
    let snap = parse_snapshot(
        &PyStr::from_str(SNAPSHOT_TEXT),
        &PyStr::from_str("snap.txt"),
    )
    .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let document = build_estate_plan(&snap, &verdicts, None).expect("estate");
    for entry_id in ["stale-1", "stale-2"] {
        let single =
            build_plan(&snap, &verdicts, &PyStr::from_str(entry_id), None).expect("single plan");
        assert!(
            document.contains(&single),
            "the block for {entry_id} is not its single plan document"
        );
    }
}

#[test]
fn the_estate_prints_its_blocks_in_scan_order_not_recorded_order() {
    // Two proven zombies recorded with the later identifier
    // first. The scan lists proven zombies by identifier, so
    // the estate prints alpha-1 ahead of zeta-1.
    let text = "snapshot 100000\nentry zeta-1\nkind process\nstarter cron-1\nstate running\nsignal-interval 60\nlast-signal 99000\nholds none\nowner ghost-7\nentry alpha-1\nkind process\nstarter cron-1\nstate running\nsignal-interval 120\nlast-signal 96400\nholds cache\nowner ghost-7\n";
    let document = estate(text, None).expect("estate");
    let alpha = document
        .find("quarantine plan for alpha-1")
        .expect("alpha block stands");
    let zeta = document
        .find("quarantine plan for zeta-1")
        .expect("zeta block stands");
    assert!(alpha < zeta, "blocks stand in recorded order");
}

// ZH-38. The skip rule and the empty form.

#[test]
fn the_estate_skips_every_entry_that_is_not_a_proven_zombie() {
    let document = estate(SNAPSHOT_TEXT, None).expect("estate");
    assert_eq!(document.matches("quarantine plan for").count(), 2);
    for entry_id in [
        "live-1", "boss-run", "done-run", "quiet-1", "busy-1", "half-1",
    ] {
        assert!(
            !document.contains(&format!("quarantine plan for {entry_id}")),
            "an unproven entry {entry_id} carries a block"
        );
    }
}

#[test]
fn an_estate_with_no_proven_zombies_is_the_framing_alone() {
    let text = "snapshot 5000\nentry live-1\nkind process\nstarter s-1\nstate running\nsignal-interval 60\nlast-signal 4990\nholds none\nowner none\nentry done-1\nkind run\nstarter s-1\nstate finished\nsignal-interval 60\nlast-signal 4000\nholds none\nowner none\n";
    assert_eq!(
        estate(text, None).expect("estate"),
        format!("estate quarantine plan\n{CLOSING}")
    );
}

// ZH-40. The seam over the estate.

#[test]
fn the_estate_asks_once_per_proven_zombie_in_scan_order() {
    let source = TableSource::new(vec![
        ("stale-1".to_string(), Some(v(QUARANTINE_JSON))),
        ("stale-2".to_string(), Some(v(ABSTENTION_JSON))),
    ]);
    let document = estate(SNAPSHOT_TEXT, Some(&source)).expect("estate");
    // The trusted quarantine answer marks stale-1 and only
    // stale-1, so the whole document is the framed pair of
    // the advanced stale-1 plan and the untouched stale-2
    // plan.
    let expected = format!("estate quarantine plan\n{ADVANCED_PLAN}{STALE_2_PLAN}{CLOSING}");
    assert_eq!(document, expected);
    // The questions stand recorded in scan order, and each
    // is the request the one builder makes from that entry
    // verdict alone.
    let snap = parse_snapshot(
        &PyStr::from_str(SNAPSHOT_TEXT),
        &PyStr::from_str("snap.txt"),
    )
    .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let recorded = source.requests.borrow();
    assert_eq!(recorded.len(), 2, "one question per proven zombie");
    for (request, entry_id) in recorded.iter().zip(["stale-1", "stale-2"]) {
        let verdict = verdicts
            .iter()
            .find(|held| held.entry.id == PyStr::from_str(entry_id))
            .expect("verdict stands");
        assert_eq!(canonical(request), canonical(&kill_call_request(verdict)));
    }
}

#[test]
fn the_estate_without_a_source_asks_nothing_and_marks_nothing() {
    let document = estate(SNAPSHOT_B, None).expect("estate");
    assert_eq!(document.matches("quarantine plan for").count(), 3);
    assert!(!document.contains("kill call verdict"));
}

// ZH-39. Snapshot level refusals and the window check stand
// ahead of every question.

#[test]
fn a_window_past_the_digit_limit_raises_before_any_question() {
    // aaa-1 is an ordinary proven zombie whose question a
    // block by block build would ask first. zzz-9 carries an
    // interval whose threshold prints at the digit limit and
    // whose window passes it, so the estate must raise ahead
    // of the first question, with no question recorded.
    let interval = format!("1{}", "0".repeat(4299));
    let time = format!("4{}", "0".repeat(4299));
    let text = format!(
        "snapshot {time}\nentry aaa-1\nkind process\nstarter cron-1\nstate running\nsignal-interval 60\nlast-signal 99000\nholds none\nowner ghost-7\nentry zzz-9\nkind process\nstarter cron-1\nstate running\nsignal-interval {interval}\nlast-signal 0\nholds none\nowner ghost-7\n"
    );
    let source = TableSource::new(vec![("aaa-1".to_string(), Some(v(QUARANTINE_JSON)))]);
    match estate(&text, Some(&source)) {
        Err(Error::Uncaught(line)) => {
            assert!(line.contains("computed count"), "{line:?}");
        }
        other => panic!("expected the uncaught family, got {other:?}"),
    }
    assert_eq!(
        source.requests.borrow().len(),
        0,
        "a question was asked ahead of the window check"
    );
}

// The command line forms of the estate.

#[test]
fn the_cli_estate_form_prints_the_estate_document() {
    let dir = common::test_dir("estate-cli");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let snap = snap.to_str().expect("utf8");
    let expected = format!("estate quarantine plan\n{EXPECTED_PLAN}{STALE_2_PLAN}{CLOSING}");
    for argv in [
        vec!["plan", snap],
        vec!["plan", "--", snap],
        vec!["plan", snap, "--"],
    ] {
        let outcome = common::run_cli(&argv, &dir);
        assert_eq!(outcome.code, 0, "{argv:?}");
        assert_eq!(outcome.stdout_text(), expected, "{argv:?}");
        assert_eq!(outcome.stderr_text(), "", "{argv:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_estate_form_takes_the_fixtures_option_in_any_position() {
    let dir = common::test_dir("estate-cli-fixtures");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_B);
    let snap = snap.to_str().expect("utf8");
    let fixtures = root().join("fixtures");
    let fixtures = fixtures.to_str().expect("utf8").to_string();
    // The library estate over the same player is the document
    // the command line must print, so the wiring is pinned
    // against the builder and not against a restatement.
    let snap_obj = parse_snapshot(&PyStr::from_str(SNAPSHOT_B), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    let player = FixturePlayer::load(fixtures.as_bytes()).expect("fixtures load");
    let expected = build_estate_plan(
        &snap_obj,
        &classify(&snap_obj).expect("classification stands"),
        Some(&player as &dyn JudgmentSource),
    )
    .expect("estate");
    assert!(expected.contains("kill call verdict spare with confidence 0.75"));
    assert_eq!(expected.matches("kill call verdict").count(), 1);
    let joined = format!("--fixtures={fixtures}");
    for argv in [
        vec!["plan", snap, "--fixtures", &fixtures],
        vec!["plan", "--fixtures", &fixtures, snap],
        vec!["plan", snap, &joined],
    ] {
        let outcome = common::run_cli(&argv, &dir);
        assert_eq!(outcome.code, 0, "{argv:?}");
        assert_eq!(outcome.stdout_text(), expected, "{argv:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_estate_form_refuses_at_the_snapshot_level_before_any_document() {
    let dir = common::test_dir("estate-cli-refusals");
    let bad = common::write(&dir, "bad.txt", "snapshot 100\nentry a\nkind daemon\n");
    let bad = bad.to_str().expect("utf8");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let snap = snap.to_str().expect("utf8");
    let outcome = common::run_cli(&["plan", bad, "--fixtures", "absent-dir"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert!(
        outcome.stderr_text().contains("malformed"),
        "{:?}",
        outcome.stderr_text()
    );
    let outcome = common::run_cli(&["plan", snap, "--fixtures", "absent-dir"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert!(
        outcome
            .stderr_text()
            .contains("fixtures directory cannot be read"),
        "{:?}",
        outcome.stderr_text()
    );
    let outcome = common::run_cli(&["plan", snap, "--fixtures", "absent-dir", "--"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_estate_form_keeps_the_trailing_separator_veto() {
    let dir = common::test_dir("estate-cli-veto");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let snap = snap.to_str().expect("utf8");
    let fixtures = root().join("fixtures");
    let fixtures = fixtures.to_str().expect("utf8").to_string();
    let outcome = common::run_cli(&["plan", snap, "--fixtures", &fixtures, "--"], &dir);
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_empty_estate_exits_zero_with_the_framing_alone() {
    let dir = common::test_dir("estate-cli-empty");
    let text = "snapshot 5000\nentry live-1\nkind process\nstarter s-1\nstate running\nsignal-interval 60\nlast-signal 4990\nholds none\nowner none\n";
    let snap = common::write(&dir, "snap.txt", text);
    let snap = snap.to_str().expect("utf8");
    let outcome = common::run_cli(&["plan", snap], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(
        outcome.stdout_text(),
        format!("estate quarantine plan\n{CLOSING}")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plan_help_names_the_entry_identifier_as_optional() {
    let dir = common::test_dir("estate-cli-help");
    let outcome = common::run_cli(&["plan", "--help"], &dir);
    assert_eq!(outcome.code, 0);
    assert!(
        outcome.stdout_text().contains("[ENTRY_ID]"),
        "{:?}",
        outcome.stdout_text()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
