//! ZH-9 and ZH-10. The quarantine plan and its refusals.

mod common;

use zombie_hunt::errors::Error;
use zombie_hunt::plan::build_plan;
use zombie_hunt::pytext::PyStr;
use zombie_hunt::scan::classify;
use zombie_hunt::snapshot::parse_snapshot;

use common::{EXPECTED_PLAN, SNAPSHOT_TEXT};

fn plan(entry_id: &str, text: &str) -> zombie_hunt::errors::Result<String> {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    build_plan(&snap, &classify(&snap), &PyStr::from_str(entry_id), None)
}

#[test]
fn the_plan_for_a_proven_zombie_matches_the_expected_document() {
    assert_eq!(plan("stale-1", SNAPSHOT_TEXT).expect("plan"), EXPECTED_PLAN);
}

#[test]
fn every_step_in_the_plan_is_marked_reversible() {
    let text = plan("stale-1", SNAPSHOT_TEXT).expect("plan");
    let steps: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("step "))
        .collect();
    assert_eq!(steps.len(), 3);
    for step in steps {
        assert!(step.contains(" reversible, "), "step {step:?} is unmarked");
    }
}

#[test]
fn the_observation_window_is_ten_signal_intervals() {
    assert!(
        plan("stale-1", SNAPSHOT_TEXT)
            .expect("plan")
            .contains("observation window 1200 seconds")
    );
    assert!(
        plan("stale-2", SNAPSHOT_TEXT)
            .expect("plan")
            .contains("observation window 600 seconds")
    );
}

#[test]
fn the_plan_for_a_zombie_holding_nothing_records_that_fence() {
    let text = plan("stale-2", SNAPSHOT_TEXT).expect("plan");
    assert!(text.contains("record that the entry holds nothing"));
}

#[test]
fn a_plan_for_an_absent_identifier_is_refused_naming_the_absence() {
    match plan("no-such-entry", SNAPSHOT_TEXT) {
        Err(Error::Plan(message)) => {
            assert!(message.contains("no-such-entry"));
            assert!(message.contains("no entry"));
        }
        other => panic!("expected a plan refusal, got {other:?}"),
    }
}

#[test]
fn a_plan_for_an_unproven_entry_is_refused_naming_its_class() {
    for (entry_id, classification) in [
        ("live-1", "live"),
        ("busy-1", "suspect"),
        ("done-run", "settled"),
    ] {
        match plan(entry_id, SNAPSHOT_TEXT) {
            Err(Error::Plan(message)) => {
                assert!(message.contains(entry_id), "{message:?}");
                assert!(
                    message.contains(&format!("classified {classification}")),
                    "{message:?}"
                );
            }
            other => panic!("expected a plan refusal, got {other:?}"),
        }
    }
}
