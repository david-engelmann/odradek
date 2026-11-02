"""ZH-14 to ZH-19. The kill call, its fixtures, and its plan rule."""

import json
import pathlib
import subprocess
import sys

import pytest

from zombie_hunt.errors import JudgmentError, PlanError
from zombie_hunt.judgment import (
    FixturePlayer,
    check_answer,
    kill_call_request,
    trusted,
    validate_fixture,
)
from zombie_hunt.plan import build_plan
from zombie_hunt.scan import classify, format_scan
from zombie_hunt.snapshot import parse_snapshot
from tests.test_plan import EXPECTED_PLAN
from tests.test_scan import EXPECTED_SCAN, SNAPSHOT

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"

SNAPSHOT_B = """snapshot 500000
entry orphan-9
kind run
starter cron-9
state running
signal-interval 45
last-signal 499100
holds queue-token
owner gone-1
entry derelict-3
kind process
starter scheduler-3
state running
signal-interval 200
last-signal 496000
holds none
owner gone-1
entry derelict-4
kind process
starter scheduler-3
state running
signal-interval 25
last-signal 499000
holds ledger
owner closer-2
entry closer-2
kind run
starter supervisor-3
state finished
signal-interval 60
last-signal 490000
holds none
owner none
"""

STALE_1_REQUEST = {
    "entry": "stale-1",
    "signal": {
        "outcome": "satisfied",
        "evidence": (
            "signal age 3600 seconds reaches the proof threshold "
            "of 360 seconds"
        ),
    },
    "owner": {
        "outcome": "satisfied",
        "evidence": "owner done-run is recorded finished",
    },
    "in-flight": {
        "outcome": "satisfied",
        "evidence": "holds cache with no in-flight marker",
    },
}

QUARANTINE = {
    "kind": "choice",
    "choice": "quarantine",
    "confidence": 0.9,
    "probabilities": {"quarantine": 0.9, "watch": 0.07, "spare": 0.03},
}

ADVANCED_PLAN = """quarantine plan for stale-1
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
kill call verdict quarantine with confidence 0.9
verdict advances this plan, the plan is ready for an operator to approve, and the approval itself stays an operator decision the tool never takes
executes nothing, this plan is a document and the tool performs no part of it
"""

WATCH_PLAN = """quarantine plan for stale-2
classification proven zombie
proof
  evidence for signal age 1000 seconds reaches the proof threshold of 180 seconds
  evidence for owner ghost-7 is absent from the snapshot
  evidence for the entry holds nothing and carries no in-flight marker
step 1 reversible, detach stale-2 from cron-1 so no new work reaches it
step 2 reversible, record that the entry holds nothing, so no resource needs fencing while the entry is watched
step 3 reversible, hold the quarantine for the observation window and watch for any signal from stale-2
observation window 600 seconds
escalation, if stale-2 shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself
kill call verdict watch with confidence 0.8
verdict leaves this plan unadvanced, no action follows from this answer, and the plan stands as written for an operator to read
executes nothing, this plan is a document and the tool performs no part of it
"""


def verdict_for(text, entry_id):
    snap = parse_snapshot(text, "snap.txt")
    return next(
        v for v in classify(snap) if v.entry.id == entry_id
    ), snap


def pair(request=None, response=None):
    return {
        "synthetic": True,
        "judgment_point": "kill-call",
        "request": STALE_1_REQUEST if request is None else request,
        "response": QUARANTINE if response is None else response,
    }


class StubSource:
    """A judgment source that records every request it is asked."""

    def __init__(self, answer=None):
        self.answer = answer
        self.requests = []

    def judge(self, request):
        self.requests.append(request)
        return self.answer


def plan_with(entry_id, source, text=SNAPSHOT):
    snap = parse_snapshot(text, "snap.txt")
    return build_plan(snap, classify(snap), entry_id, source)


# ZH-14. The request is the proof pack and nothing else.


def test_the_kill_call_request_carries_the_proof_pack():
    verdict, _ = verdict_for(SNAPSHOT, "stale-1")
    assert kill_call_request(verdict) == STALE_1_REQUEST


def test_the_request_is_built_from_the_verdict_alone():
    first, _ = verdict_for(SNAPSHOT, "stale-1")
    second, _ = verdict_for(SNAPSHOT, "stale-1")
    assert kill_call_request(first) == kill_call_request(second)
    key = json.dumps(kill_call_request(first), sort_keys=True)
    assert json.loads(key) == STALE_1_REQUEST


# ZH-15. The fixture shape, refused at load defect by defect.


def test_a_well_formed_fixture_validates():
    assert validate_fixture(pair())["response"] == QUARANTINE


def test_probabilities_within_the_sum_tolerance_validate():
    response = dict(
        QUARANTINE,
        probabilities={
            "quarantine": 0.334,
            "watch": 0.333,
            "spare": 0.333,
        },
    )
    assert validate_fixture(pair(response=response)) is not None


def _mutated(mutator):
    data = json.loads(json.dumps(pair()))
    mutator(data)
    return data


@pytest.mark.parametrize(
    "mutator, fragment",
    [
        (lambda d: d.update(comment="recorded by hand"), "'comment'"),
        (lambda d: d.pop("response"), "carries no response"),
        (lambda d: d.update(synthetic=False), "not marked synthetic"),
        (
            lambda d: d.update(judgment_point="classification"),
            "not the kill-call point",
        ),
        (
            lambda d: d["request"].update(extra="x"),
            "does not carry exactly the fields",
        ),
        (
            lambda d: d["request"]["signal"].pop("evidence"),
            "outcome and evidence",
        ),
        (
            lambda d: d["request"]["owner"].update(outcome="proven"),
            "not satisfied, contradicted, or unknown",
        ),
        (
            lambda d: d["response"].update(notes="sure"),
            "'notes'",
        ),
        (
            lambda d: d["response"].update(
                kind="abstention", confidence=0.9
            ),
            "'choice'",
        ),
        (
            lambda d: d["response"].update(choice="kill"),
            "not one of the labels",
        ),
        (
            lambda d: d["response"]["probabilities"].pop("spare"),
            "do not cover exactly the labels",
        ),
        (
            lambda d: d["response"]["probabilities"].update(extra=0.0),
            "do not cover exactly the labels",
        ),
        (
            lambda d: d["response"].update(
                probabilities={
                    "quarantine": 0.9,
                    "watch": 0.07,
                    "spare": 0.02,
                }
            ),
            "do not add up to 1",
        ),
        (
            lambda d: d["response"].update(confidence="0.9"),
            "not a number between 0 and 1",
        ),
        (
            lambda d: d["response"].update(confidence=1.5),
            "not a number between 0 and 1",
        ),
    ],
)
def test_a_fixture_outside_the_shape_is_refused_at_load(mutator, fragment):
    with pytest.raises(JudgmentError) as caught:
        validate_fixture(_mutated(mutator))
    assert fragment in str(caught.value)


def test_a_non_string_kind_is_refused_at_load_and_is_no_judgment():
    bad = dict(QUARANTINE, kind=[])
    with pytest.raises(JudgmentError):
        validate_fixture(pair(response=bad))
    assert check_answer(bad) is None


def test_an_unconvertible_number_is_refused_and_is_no_judgment():
    bad = dict(QUARANTINE, confidence=10**400)
    with pytest.raises(JudgmentError):
        validate_fixture(pair(response=bad))
    assert check_answer(bad) is None
    nan = dict(QUARANTINE, confidence=float("nan"))
    assert check_answer(nan) is None


# ZH-16. The player loads, validates, matches exactly, and abstains.


def write_fixtures(tmp_path, pairs):
    for index, data in enumerate(pairs):
        path = tmp_path / f"fixture-{index}.json"
        path.write_text(json.dumps(data), encoding="utf-8")
    return str(tmp_path)


def test_the_player_answers_a_recorded_request_and_abstains_elsewhere(
    tmp_path,
):
    player = FixturePlayer(write_fixtures(tmp_path, [pair()]))
    assert player.judge(STALE_1_REQUEST) == QUARANTINE
    other = dict(STALE_1_REQUEST, entry="stale-2")
    assert player.judge(other) is None


def test_two_fixtures_for_one_request_refuse_the_load(tmp_path):
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(write_fixtures(tmp_path, [pair(), pair()]))
    assert "records the same request" in str(caught.value)


def test_a_missing_fixtures_directory_is_refused_at_load(tmp_path):
    with pytest.raises(JudgmentError):
        FixturePlayer(str(tmp_path / "absent"))


def test_a_fixture_that_is_not_json_is_refused_at_load(tmp_path):
    path = tmp_path / "fixture-0.json"
    path.write_text("not json", encoding="utf-8")
    with pytest.raises(JudgmentError):
        FixturePlayer(str(tmp_path))


def test_the_committed_fixtures_load_and_cover_the_five_answers():
    player = FixturePlayer(str(FIXTURES))
    stale_1, _ = verdict_for(SNAPSHOT, "stale-1")
    stale_2, _ = verdict_for(SNAPSHOT, "stale-2")
    orphan, _ = verdict_for(SNAPSHOT_B, "orphan-9")
    derelict_3, _ = verdict_for(SNAPSHOT_B, "derelict-3")
    derelict_4, _ = verdict_for(SNAPSHOT_B, "derelict-4")
    assert player.judge(kill_call_request(stale_1))["choice"] == "quarantine"
    assert player.judge(kill_call_request(stale_2))["choice"] == "watch"
    spare = player.judge(kill_call_request(orphan))
    assert spare["choice"] == "spare"
    assert spare["confidence"] == 0.75
    assert player.judge(kill_call_request(derelict_3)) == {
        "kind": "abstention"
    }
    below = player.judge(kill_call_request(derelict_4))
    assert below["choice"] == "quarantine"
    assert below["confidence"] == 0.45
    assert not trusted(below)


# ZH-17. Trust stands at the threshold and nowhere else.


def test_trust_reaches_at_exactly_point_six():
    at = dict(QUARANTINE, confidence=0.6)
    under = dict(QUARANTINE, confidence=0.599999)
    assert trusted(at)
    assert not trusted(under)
    assert check_answer(under) == under
    assert not trusted({"kind": "abstention"})
    assert not trusted(None)


# ZH-18. The plan rule, asked late, recorded exactly, or untouched.


@pytest.mark.parametrize(
    "entry_id", ["live-1", "busy-1", "done-run", "no-such-entry"]
)
def test_a_refused_plan_asks_nothing(entry_id):
    spy = StubSource(QUARANTINE)
    snap = parse_snapshot(SNAPSHOT, "snap.txt")
    with pytest.raises(PlanError):
        build_plan(snap, classify(snap), entry_id, spy)
    assert spy.requests == []


def test_a_trusted_quarantine_answer_advances_the_plan():
    spy = StubSource(QUARANTINE)
    assert plan_with("stale-1", spy) == ADVANCED_PLAN
    assert spy.requests == [STALE_1_REQUEST]


def test_a_trusted_watch_answer_is_recorded_unadvanced():
    watch = {
        "kind": "choice",
        "choice": "watch",
        "confidence": 0.8,
        "probabilities": {
            "quarantine": 0.1,
            "watch": 0.8,
            "spare": 0.1,
        },
    }
    assert plan_with("stale-2", StubSource(watch)) == WATCH_PLAN
    assert "advances this plan" not in plan_with("stale-2", StubSource(watch))


def test_a_trusted_spare_answer_is_recorded_unadvanced():
    spare = {
        "kind": "choice",
        "choice": "spare",
        "confidence": 0.75,
        "probabilities": {
            "quarantine": 0.15,
            "watch": 0.1,
            "spare": 0.75,
        },
    }
    text = plan_with("stale-1", StubSource(spare))
    assert "kill call verdict spare with confidence 0.75" in text
    assert "verdict leaves this plan unadvanced" in text
    assert "advances this plan" not in text
    assert text.endswith(
        "executes nothing, this plan is a document and the "
        "tool performs no part of it\n"
    )


def test_an_abstention_leaves_the_round_1_plan_byte_identical():
    spy = StubSource({"kind": "abstention"})
    assert plan_with("stale-1", spy) == EXPECTED_PLAN
    assert spy.requests == [STALE_1_REQUEST]


def test_a_below_threshold_answer_leaves_the_round_1_plan():
    below = dict(
        QUARANTINE,
        confidence=0.45,
        probabilities={
            "quarantine": 0.45,
            "watch": 0.35,
            "spare": 0.2,
        },
    )
    assert plan_with("stale-1", StubSource(below)) == EXPECTED_PLAN


def test_an_answer_at_exactly_point_six_advances_the_plan():
    at = dict(QUARANTINE, confidence=0.6)
    text = plan_with("stale-1", StubSource(at))
    assert "kill call verdict quarantine with confidence 0.6" in text
    assert "verdict advances this plan" in text


@pytest.mark.parametrize(
    "answer",
    [
        dict(QUARANTINE, kind=[]),
        dict(QUARANTINE, confidence=10**400),
        dict(QUARANTINE, notes="sure"),
        dict(QUARANTINE, choice="kill"),
        {"kind": "score", "score": 0.9, "confidence": 0.9},
        "quarantine",
        None,
    ],
)
def test_a_hostile_answer_counts_as_no_judgment(answer):
    assert plan_with("stale-1", StubSource(answer)) == EXPECTED_PLAN


def test_the_verdict_never_touches_the_plan_above_it():
    text = plan_with("stale-1", StubSource(QUARANTINE))
    head = EXPECTED_PLAN.split("executes nothing")[0]
    assert text.startswith(head)
    assert text.count("step ") == EXPECTED_PLAN.count("step ")


def test_the_committed_player_drives_the_plan_end_to_end():
    player = FixturePlayer(str(FIXTURES))
    assert plan_with("stale-1", player) == ADVANCED_PLAN
    assert plan_with("stale-2", player) == WATCH_PLAN
    for entry_id in ("orphan-9", "derelict-3", "derelict-4"):
        with_verdict = plan_with(entry_id, player, SNAPSHOT_B)
        snap = parse_snapshot(SNAPSHOT_B, "snap.txt")
        plain = build_plan(snap, classify(snap), entry_id)
        if entry_id == "orphan-9":
            assert "kill call verdict spare" in with_verdict
            assert with_verdict != plain
        else:
            assert with_verdict == plain


# ZH-19 and the CLI. The scan never consults, load failures refuse.


def run_cli(*args):
    return subprocess.run(
        [sys.executable, "-m", "zombie_hunt", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def write_snapshot(tmp_path, text=SNAPSHOT):
    path = tmp_path / "snap.txt"
    path.write_text(text, encoding="utf-8")
    return str(path)


def test_the_scan_is_byte_identical_with_the_seam_built(tmp_path):
    snap = parse_snapshot(SNAPSHOT, "snap.txt")
    assert format_scan(snap, classify(snap)) == EXPECTED_SCAN
    result = run_cli("scan", write_snapshot(tmp_path))
    assert result.returncode == 0
    assert result.stdout == EXPECTED_SCAN


def test_the_cli_plan_without_fixtures_matches_round_1(tmp_path):
    result = run_cli("plan", write_snapshot(tmp_path), "stale-1")
    assert result.returncode == 0
    assert result.stdout == EXPECTED_PLAN
    assert result.stderr == ""


def test_the_cli_plan_with_fixtures_records_the_verdicts(tmp_path):
    snap = write_snapshot(tmp_path)
    advanced = run_cli("plan", snap, "stale-1", "--fixtures", str(FIXTURES))
    assert advanced.returncode == 0
    assert advanced.stdout == ADVANCED_PLAN
    watched = run_cli("plan", snap, "stale-2", "--fixtures", str(FIXTURES))
    assert watched.returncode == 0
    assert watched.stdout == WATCH_PLAN


def test_the_cli_plan_refusal_stands_with_fixtures_present(tmp_path):
    result = run_cli(
        "plan",
        write_snapshot(tmp_path),
        "busy-1",
        "--fixtures",
        str(FIXTURES),
    )
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "classified suspect" in result.stderr


def test_a_missing_fixtures_directory_exits_two_with_one_line(tmp_path):
    result = run_cli(
        "plan",
        write_snapshot(tmp_path),
        "stale-1",
        "--fixtures",
        str(tmp_path / "absent"),
    )
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "Traceback" not in result.stderr


def test_a_malformed_fixture_exits_two_with_one_line(tmp_path):
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps(pair(response=dict(QUARANTINE, choice="kill"))))
    result = run_cli(
        "plan",
        write_snapshot(tmp_path),
        "stale-1",
        "--fixtures",
        str(tmp_path),
    )
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "Traceback" not in result.stderr
