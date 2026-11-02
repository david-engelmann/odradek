"""ZH-5 to ZH-8. Classification, the scan document, and determinism."""

import re

from zombie_hunt.scan import classify, format_scan
from zombie_hunt.snapshot import parse_snapshot

SNAPSHOT = """snapshot 100000
entry live-1
kind process
starter scheduler-1
state running
signal-interval 60
last-signal 99980
holds workspace
owner boss-run
entry boss-run
kind run
starter supervisor-1
state running
signal-interval 30
last-signal 99995
holds none
owner none
entry done-run
kind run
starter supervisor-1
state finished
signal-interval 300
last-signal 90000
holds none
owner none
entry stale-1
kind process
starter scheduler-1
state running
signal-interval 120
last-signal 96400
holds cache
owner done-run
entry stale-2
kind process
starter cron-1
state running
signal-interval 60
last-signal 99000
holds none
owner ghost-7
entry quiet-1
kind process
starter scheduler-1
state running
signal-interval 90
last-signal none
holds none
owner none
entry busy-1
kind process
starter scheduler-1
state running
signal-interval 60
last-signal 99850
holds in-flight,cache
owner boss-run
entry half-1
kind process
starter cron-1
state running
signal-interval 300
last-signal 95000
holds reports
owner none
"""

EXPECTED_SCAN = """snapshot 100000
counts live 2 suspect 3 proven 2 settled 1
zombie stale-1
  evidence for signal age 3600 seconds reaches the proof threshold of 360 seconds
  evidence for owner done-run is recorded finished
  evidence for holds cache with no in-flight marker
zombie stale-2
  evidence for signal age 1000 seconds reaches the proof threshold of 180 seconds
  evidence for owner ghost-7 is absent from the snapshot
  evidence for the entry holds nothing and carries no in-flight marker
suspect busy-1
  evidence against signal age 150 seconds is under the proof threshold of 180 seconds
  evidence against owner boss-run is recorded running
  evidence against holds cache, in-flight including the in-flight marker
suspect half-1
  evidence for signal age 5000 seconds reaches the proof threshold of 900 seconds
  missing no owner reference is recorded for this entry
  evidence for holds reports with no in-flight marker
suspect quiet-1
  missing no signal is recorded for this entry
  missing no owner reference is recorded for this entry
  evidence for the entry holds nothing and carries no in-flight marker
"""


def scan(text=SNAPSHOT):
    snap = parse_snapshot(text, "snap.txt")
    return format_scan(snap, classify(snap))


def test_the_scan_matches_the_expected_document_byte_for_byte():
    assert scan() == EXPECTED_SCAN


def test_no_suspect_is_ever_reported_as_a_zombie():
    snap = parse_snapshot(SNAPSHOT, "snap.txt")
    verdicts = {v.entry.id: v.classification for v in classify(snap)}
    assert verdicts["busy-1"] == "suspect"
    assert verdicts["half-1"] == "suspect"
    assert verdicts["quiet-1"] == "suspect"
    out = scan()
    zombie_ids = re.findall(r"^zombie (\S+)$", out, re.M)
    assert zombie_ids == ["stale-1", "stale-2"]


def test_live_and_settled_entries_are_counted_but_never_listed():
    out = scan()
    assert "counts live 2 suspect 3 proven 2 settled 1" in out
    listed = re.findall(r"^(?:zombie|suspect) (\S+)$", out, re.M)
    assert "live-1" not in listed
    assert "boss-run" not in listed
    assert "done-run" not in listed


def test_every_listed_entry_carries_exactly_three_evidence_lines():
    blocks = re.split(r"^(?:zombie|suspect) ", scan(), flags=re.M)[1:]
    assert len(blocks) == 5
    for block in blocks:
        evidence = [
            line
            for line in block.splitlines()
            if line.startswith("  ")
        ]
        assert len(evidence) == 3


def test_a_snapshot_with_nothing_to_list_prints_the_counts_line_alone():
    text = """snapshot 500
entry only-live
kind process
starter scheduler-1
state running
signal-interval 60
last-signal 490
holds none
owner none
"""
    assert scan(text) == "snapshot 500\ncounts live 1 suspect 0 proven 0 settled 0\n"


def test_repeated_and_shuffled_snapshots_scan_byte_identically():
    assert scan() == scan()
    blocks = SNAPSHOT.split("entry ")[1:]
    shuffled = "snapshot 100000\nentry " + "entry ".join(
        reversed(blocks)
    )
    shuffled = shuffled.replace("holds in-flight,cache", "holds cache,in-flight")
    assert scan(shuffled) == EXPECTED_SCAN
