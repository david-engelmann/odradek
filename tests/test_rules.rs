//! ZH-2, ZH-3, and ZH-4. The three proof rules at their edges.

use odradek::pytext::PyStr;
use odradek::scan::{CONTRADICTED, SATISFIED, UNKNOWN, Verdict, classify_entry};
use odradek::snapshot::parse_snapshot;

fn entry_block(entry_id: &str, overrides: &[(&str, &str)]) -> String {
    let mut fields: Vec<(&str, &str)> = vec![
        ("kind", "process"),
        ("starter", "scheduler-1"),
        ("state", "running"),
        ("signal-interval", "100"),
        ("last-signal", "99000"),
        ("holds", "none"),
        ("owner", "none"),
    ];
    for (name, value) in overrides {
        if let Some(slot) = fields.iter_mut().find(|(held, _)| held == name) {
            slot.1 = value;
        }
    }
    let mut lines = vec![format!("entry {entry_id}")];
    for (name, value) in fields {
        lines.push(format!("{name} {value}"));
    }
    lines.join("\n")
}

fn verdict_for(text: &str, entry_id: &str) -> Verdict<'static> {
    let snap = parse_snapshot(
        &PyStr::from_str(&format!("snapshot 100000\n{text}\n")),
        &PyStr::from_str("snap.txt"),
    )
    .expect("snapshot parses");
    // The verdict borrows the snapshot; leak it for the test's
    // lifetime so the helper reads like the Python one.
    let snap = Box::leak(Box::new(snap));
    let entry = snap
        .entries
        .iter()
        .find(|e| e.id == PyStr::from_str(entry_id))
        .expect("entry stands");
    classify_entry(snap, entry).expect("classification stands")
}

#[test]
fn the_signal_rule_is_satisfied_at_exactly_three_intervals() {
    let verdict = verdict_for(&entry_block("a", &[("last-signal", "99700")]), "a");
    assert_eq!(verdict.signal.status, SATISFIED);
    assert!(verdict.signal.fact.contains("300 seconds"));
}

#[test]
fn the_signal_rule_fails_one_second_short_of_the_threshold() {
    let verdict = verdict_for(&entry_block("a", &[("last-signal", "99701")]), "a");
    assert_eq!(verdict.signal.status, CONTRADICTED);
    assert!(verdict.signal.fact.contains("299 seconds"));
}

#[test]
fn the_signal_rule_is_unknown_when_no_signal_is_recorded() {
    let verdict = verdict_for(&entry_block("a", &[("last-signal", "none")]), "a");
    assert_eq!(verdict.signal.status, UNKNOWN);
}

#[test]
fn the_owner_rule_reads_the_owner_state_from_the_snapshot() {
    let text = format!(
        "{}\n{}",
        entry_block("owned", &[("owner", "boss")]),
        entry_block("boss", &[("state", "finished")])
    );
    let verdict = verdict_for(&text, "owned");
    assert_eq!(verdict.owner.status, SATISFIED);
    assert!(verdict.owner.fact.contains("recorded finished"));
}

#[test]
fn the_owner_rule_is_satisfied_when_the_owner_is_absent() {
    let verdict = verdict_for(&entry_block("a", &[("owner", "ghost-9")]), "a");
    assert_eq!(verdict.owner.status, SATISFIED);
    assert!(verdict.owner.fact.contains("absent from the snapshot"));
}

#[test]
fn the_owner_rule_is_contradicted_when_the_owner_is_running() {
    let text = format!(
        "{}\n{}",
        entry_block("owned", &[("owner", "boss")]),
        entry_block("boss", &[("state", "running")])
    );
    let verdict = verdict_for(&text, "owned");
    assert_eq!(verdict.owner.status, CONTRADICTED);
    assert!(verdict.owner.fact.contains("recorded running"));
}

#[test]
fn the_owner_rule_is_unknown_when_no_owner_is_recorded() {
    let verdict = verdict_for(&entry_block("a", &[("owner", "none")]), "a");
    assert_eq!(verdict.owner.status, UNKNOWN);
}

#[test]
fn the_in_flight_rule_is_contradicted_by_the_marker() {
    let verdict = verdict_for(&entry_block("a", &[("holds", "cache,in-flight")]), "a");
    assert_eq!(verdict.in_flight.status, CONTRADICTED);
    assert!(verdict.in_flight.fact.contains("in-flight marker"));
}

#[test]
fn the_in_flight_rule_is_satisfied_without_the_marker() {
    let verdict = verdict_for(&entry_block("a", &[("holds", "cache,reports")]), "a");
    assert_eq!(verdict.in_flight.status, SATISFIED);
    let verdict = verdict_for(&entry_block("a", &[("holds", "none")]), "a");
    assert_eq!(verdict.in_flight.status, SATISFIED);
}
