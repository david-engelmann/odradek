"""ZH-20. The holds list as a set, and the permutation invariance."""

import itertools
import pathlib

from zombie_hunt.judgment import FixturePlayer, kill_call_request
from zombie_hunt.plan import build_plan
from zombie_hunt.scan import (
    CONTRADICTED,
    PROVEN,
    SUSPECT,
    classify,
    format_scan,
)
from zombie_hunt.snapshot import parse_snapshot

ROOT = pathlib.Path(__file__).resolve().parents[1]
EXAMPLE = (ROOT / "examples" / "snapshot.txt").read_text(
    encoding="utf-8"
)
FIXTURES = ROOT / "fixtures"

STALE_HOLDS = "holds cache\nowner done-run"


def scan_document(text):
    snap = parse_snapshot(text, "snap.txt")
    return format_scan(snap, classify(snap))


def plan_document(text, entry_id, source=None):
    snap = parse_snapshot(text, "snap.txt")
    return build_plan(snap, classify(snap), entry_id, source)


def verdict_for(text, entry_id):
    snap = parse_snapshot(text, "snap.txt")
    return next(v for v in classify(snap) if v.entry.id == entry_id)


def test_a_repeated_plain_token_scans_byte_identically_to_one_token():
    assert EXAMPLE.count(STALE_HOLDS) == 1
    repeated = EXAMPLE.replace(STALE_HOLDS, "holds cache,cache\nowner done-run")
    scan = scan_document(repeated)
    assert scan == scan_document(EXAMPLE)
    assert "holds cache with no in-flight marker" in scan
    assert "cache, cache" not in scan


def test_a_repeated_plain_token_plans_byte_identically_to_one_token():
    repeated = EXAMPLE.replace(STALE_HOLDS, "holds cache,cache\nowner done-run")
    plan = plan_document(repeated, "stale-1")
    assert plan == plan_document(EXAMPLE, "stale-1")
    assert "fence the recorded holds, cache, so" in plan
    assert "cache, cache" not in plan


def test_a_repeated_token_builds_the_single_token_kill_call_request():
    repeated = EXAMPLE.replace(STALE_HOLDS, "holds cache,cache\nowner done-run")
    assert kill_call_request(
        verdict_for(repeated, "stale-1")
    ) == kill_call_request(verdict_for(EXAMPLE, "stale-1"))


def test_a_repeated_token_plan_matches_the_recorded_fixture_verdict():
    player = FixturePlayer(str(FIXTURES))
    repeated = EXAMPLE.replace(STALE_HOLDS, "holds cache,cache\nowner done-run")
    plan = plan_document(repeated, "stale-1", player)
    assert plan == plan_document(EXAMPLE, "stale-1", player)
    assert "kill call verdict quarantine with confidence 0.9" in plan
    assert "verdict advances this plan" in plan


def test_a_repeated_in_flight_marker_scans_byte_identically_to_one():
    assert EXAMPLE.count("holds in-flight,cache") == 1
    single = EXAMPLE.replace("holds in-flight,cache", "holds in-flight")
    repeated = EXAMPLE.replace(
        "holds in-flight,cache", "holds in-flight,in-flight"
    )
    scan = scan_document(repeated)
    assert scan == scan_document(single)
    assert "holds in-flight including the in-flight marker" in scan
    assert "in-flight, in-flight" not in scan


def test_an_entry_holding_its_own_identifier_is_unremarkable():
    text = EXAMPLE.replace(STALE_HOLDS, "holds stale-1\nowner done-run")
    verdict = verdict_for(text, "stale-1")
    assert verdict.classification == PROVEN
    assert verdict.in_flight.fact == (
        "holds stale-1 with no in-flight marker"
    )
    assert "zombie stale-1" in scan_document(text)


def test_an_entry_naming_itself_as_owner_is_at_most_a_suspect():
    text = EXAMPLE.replace(STALE_HOLDS, "holds cache\nowner stale-1")
    verdict = verdict_for(text, "stale-1")
    assert verdict.classification == SUSPECT
    assert verdict.owner.status == CONTRADICTED
    assert verdict.owner.fact == "owner stale-1 is recorded running"
    scan = scan_document(text)
    assert "suspect stale-1" in scan
    assert "evidence against owner stale-1 is recorded running" in scan


def split_blocks(text):
    lines = [
        line
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    blocks = []
    for line in lines[1:]:
        if line.startswith("entry "):
            blocks.append([line])
        else:
            blocks[-1].append(line)
    return lines[0], blocks


def rotated_block(block, amount):
    head, fields = block[0], list(block[1:])
    if fields:
        amount %= len(fields)
        fields = fields[amount:] + fields[:amount]
    lines = []
    for line in [head, *fields]:
        if line.startswith("holds ") and "," in line:
            name, value = line.split(" ", 1)
            line = name + " " + ",".join(reversed(value.split(",")))
        lines.append(line)
    return "\n".join(lines)


def test_all_40320_block_permutations_of_the_example_scan_identically():
    header, blocks = split_blocks(EXAMPLE)
    assert len(blocks) == 8
    baseline = scan_document(EXAMPLE)
    count = 0
    for ordinal, order in enumerate(itertools.permutations(range(8))):
        parts = [header]
        for position, block_index in enumerate(order):
            parts.append(
                rotated_block(blocks[block_index], ordinal + position)
            )
        text = "\n".join(parts) + "\n"
        assert scan_document(text) == baseline
        count += 1
    assert count == 40320


def test_all_24_block_permutations_of_a_subset_plan_identically():
    header, blocks = split_blocks(EXAMPLE)
    by_id = {block[0].split()[1]: block for block in blocks}
    chosen = [
        by_id[name]
        for name in ("live-1", "boss-run", "done-run", "stale-1")
    ]
    recorded = "\n".join(
        [header, *("\n".join(block) for block in chosen)]
    ) + "\n"
    player = FixturePlayer(str(FIXTURES))
    baseline_scan = scan_document(recorded)
    baseline_plan = plan_document(recorded, "stale-1")
    baseline_verdict_plan = plan_document(recorded, "stale-1", player)
    assert "kill call verdict quarantine" in baseline_verdict_plan
    count = 0
    for order in itertools.permutations(range(4)):
        text = "\n".join(
            [header, *("\n".join(chosen[index]) for index in order)]
        ) + "\n"
        assert scan_document(text) == baseline_scan
        assert plan_document(text, "stale-1") == baseline_plan
        assert (
            plan_document(text, "stale-1", player)
            == baseline_verdict_plan
        )
        count += 1
    assert count == 24
