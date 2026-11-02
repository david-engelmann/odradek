"""ZH-9 and ZH-10. The quarantine plan and its refusals."""

import pytest

from zombie_hunt.errors import PlanError
from zombie_hunt.plan import build_plan
from zombie_hunt.scan import classify
from zombie_hunt.snapshot import parse_snapshot
from tests.test_scan import SNAPSHOT

EXPECTED_PLAN = """quarantine plan for stale-1
classification proven zombie
proof
  evidence for signal age 3600 seconds reaches the proof threshold of 360 seconds
  evidence for owner done-run is recorded finished
  evidence for holds cache with no in-flight marker
step 1 reversible, detach stale-1 from scheduler-1 so no new work reaches it
step 2 reversible, fence the recorded holds, cache, so nothing else claims them while the entry is watched
step 3 reversible, hold the quarantine for the observation window and watch for any signal from stale-1
observation window 1200 seconds
escalation, if stale-1 shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself
executes nothing, this plan is a document and the tool performs no part of it
"""


def plan(entry_id, text=SNAPSHOT):
    snap = parse_snapshot(text, "snap.txt")
    return build_plan(snap, classify(snap), entry_id)


def test_the_plan_for_a_proven_zombie_matches_the_expected_document():
    assert plan("stale-1") == EXPECTED_PLAN


def test_every_step_in_the_plan_is_marked_reversible():
    steps = [
        line
        for line in plan("stale-1").splitlines()
        if line.startswith("step ")
    ]
    assert len(steps) == 3
    for step in steps:
        assert " reversible, " in step


def test_the_observation_window_is_ten_signal_intervals():
    assert "observation window 1200 seconds" in plan("stale-1")
    assert "observation window 600 seconds" in plan("stale-2")


def test_the_plan_for_a_zombie_holding_nothing_records_that_fence():
    text = plan("stale-2")
    assert "record that the entry holds nothing" in text


def test_a_plan_for_an_absent_identifier_is_refused_naming_the_absence():
    with pytest.raises(PlanError) as caught:
        plan("no-such-entry")
    assert "no-such-entry" in str(caught.value)
    assert "no entry" in str(caught.value)


@pytest.mark.parametrize(
    "entry_id, classification",
    [
        ("live-1", "live"),
        ("busy-1", "suspect"),
        ("done-run", "settled"),
    ],
)
def test_a_plan_for_an_unproven_entry_is_refused_naming_its_class(
    entry_id, classification
):
    with pytest.raises(PlanError) as caught:
        plan(entry_id)
    assert entry_id in str(caught.value)
    assert f"classified {classification}" in str(caught.value)
