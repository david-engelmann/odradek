"""ZH-2, ZH-3, and ZH-4. The three proof rules at their edges."""

from zombie_hunt.scan import (
    CONTRADICTED,
    SATISFIED,
    UNKNOWN,
    classify_entry,
)
from zombie_hunt.snapshot import parse_snapshot


def entry_block(entry_id, **overrides):
    fields = {
        "kind": "process",
        "starter": "scheduler-1",
        "state": "running",
        "signal-interval": "100",
        "last-signal": "99000",
        "holds": "none",
        "owner": "none",
    }
    fields.update(overrides)
    lines = [f"entry {entry_id}"]
    lines += [f"{name} {value}" for name, value in fields.items()]
    return "\n".join(lines)


def verdict_for(text, entry_id):
    snap = parse_snapshot("snapshot 100000\n" + text + "\n", "snap.txt")
    return classify_entry(snap, snap.by_id[entry_id])


def test_the_signal_rule_is_satisfied_at_exactly_three_intervals():
    verdict = verdict_for(entry_block("a", **{"last-signal": "99700"}), "a")
    assert verdict.signal.status == SATISFIED
    assert "300 seconds" in verdict.signal.fact


def test_the_signal_rule_fails_one_second_short_of_the_threshold():
    verdict = verdict_for(entry_block("a", **{"last-signal": "99701"}), "a")
    assert verdict.signal.status == CONTRADICTED
    assert "299 seconds" in verdict.signal.fact


def test_the_signal_rule_is_unknown_when_no_signal_is_recorded():
    verdict = verdict_for(entry_block("a", **{"last-signal": "none"}), "a")
    assert verdict.signal.status == UNKNOWN


def test_the_owner_rule_reads_the_owner_state_from_the_snapshot():
    text = (
        entry_block("owned", owner="boss")
        + "\n"
        + entry_block("boss", state="finished")
    )
    verdict = verdict_for(text, "owned")
    assert verdict.owner.status == SATISFIED
    assert "recorded finished" in verdict.owner.fact


def test_the_owner_rule_is_satisfied_when_the_owner_is_absent():
    verdict = verdict_for(entry_block("a", owner="ghost-9"), "a")
    assert verdict.owner.status == SATISFIED
    assert "absent from the snapshot" in verdict.owner.fact


def test_the_owner_rule_is_contradicted_when_the_owner_is_running():
    text = (
        entry_block("owned", owner="boss")
        + "\n"
        + entry_block("boss", state="running")
    )
    verdict = verdict_for(text, "owned")
    assert verdict.owner.status == CONTRADICTED
    assert "recorded running" in verdict.owner.fact


def test_the_owner_rule_is_unknown_when_no_owner_is_recorded():
    verdict = verdict_for(entry_block("a", owner="none"), "a")
    assert verdict.owner.status == UNKNOWN


def test_the_in_flight_rule_is_contradicted_by_the_marker():
    verdict = verdict_for(
        entry_block("a", holds="cache,in-flight"), "a"
    )
    assert verdict.in_flight.status == CONTRADICTED
    assert "in-flight marker" in verdict.in_flight.fact


def test_the_in_flight_rule_is_satisfied_without_the_marker():
    verdict = verdict_for(entry_block("a", holds="cache,reports"), "a")
    assert verdict.in_flight.status == SATISFIED
    verdict = verdict_for(entry_block("a", holds="none"), "a")
    assert verdict.in_flight.status == SATISFIED
