"""ZH-1. The recorded snapshot format and its refusals."""

import pytest

from zombie_hunt.errors import SnapshotError
from zombie_hunt.snapshot import parse_snapshot

VALID = """snapshot 100000
entry proc-1
kind process
starter scheduler-1
state running
signal-interval 60
last-signal 99980
holds cache,reports
owner run-1
entry run-1
kind run
starter supervisor-1
state running
signal-interval 30
last-signal 99995
holds none
owner none
"""


def test_a_valid_snapshot_parses_with_its_facts_intact():
    snap = parse_snapshot(VALID, "snap.txt")
    assert snap.time == 100000
    assert [entry.id for entry in snap.entries] == ["proc-1", "run-1"]
    proc = snap.by_id["proc-1"]
    assert proc.kind == "process"
    assert proc.starter == "scheduler-1"
    assert proc.signal_interval == 60
    assert proc.last_signal == 99980
    assert proc.holds == ("cache", "reports")
    assert proc.owner == "run-1"
    run = snap.by_id["run-1"]
    assert run.holds == ()
    assert run.owner is None


def test_comments_blank_lines_and_field_order_change_nothing():
    shuffled = """# a recorded snapshot, synthetic

snapshot 100000
entry proc-1
owner run-1
holds cache,reports
last-signal 99980
signal-interval 60
state running
starter scheduler-1
kind process
entry run-1
kind run
starter supervisor-1
state running
signal-interval 30
last-signal 99995
holds none
owner none
"""
    assert parse_snapshot(shuffled, "snap.txt") == parse_snapshot(
        VALID, "snap.txt"
    )


REFUSALS = [
    ("", "no snapshot header"),
    ("entry proc-1\nkind process\n", "not a snapshot header"),
    ("snapshot 100000\nsnapshot 100001\n", "more than once"),
    ("snapshot soon\n", "not a whole number"),
    ("snapshot 100000\nkind process\n", "before any entry block"),
    (
        "snapshot 100000\nentry proc-1 extra\n",
        "exactly one identifier",
    ),
    (
        "snapshot 100000\nentry proc-1\nkind process\nkind run\n",
        "carries the kind line twice",
    ),
    (
        "snapshot 100000\nentry proc-1\nweight heavy\n",
        "matches no field form",
    ),
    (
        "snapshot 100000\nentry proc-1\nkind process\n",
        "carries no starter line",
    ),
    (
        VALID + "entry proc-1\nkind process\n",
        "is entered twice",
    ),
    (
        VALID.replace("signal-interval 60", "signal-interval 0"),
        "not positive",
    ),
    (
        VALID.replace("signal-interval 60", "signal-interval soon"),
        "not a whole number of seconds",
    ),
    (
        VALID.replace("last-signal 99980", "last-signal 100001"),
        "later than the snapshot time",
    ),
    (VALID.replace("kind process", "kind daemon"), "neither process nor run"),
    (VALID.replace("state running", "state paused", 1), "neither running nor finished"),
    (
        VALID.replace("holds cache,reports", "holds none,cache"),
        "the word none inside the list",
    ),
    (
        VALID.replace("holds cache,reports", "holds cache,,reports"),
        "an empty token",
    ),
]


@pytest.mark.parametrize("text, fragment", REFUSALS)
def test_a_malformed_snapshot_is_refused_with_the_defect_named(text, fragment):
    with pytest.raises(SnapshotError) as caught:
        parse_snapshot(text, "snap.txt")
    assert fragment in str(caught.value)
    assert "snap.txt" in str(caught.value)
