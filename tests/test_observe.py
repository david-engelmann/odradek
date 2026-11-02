"""ZH-21 to ZH-24. The observation report over the plan's watch."""

import pathlib
import subprocess
import sys

import pytest

from zombie_hunt import build_observation, classify, parse_snapshot
from zombie_hunt.errors import ObserveError
from zombie_hunt.scan import classify_entry

ROOT = pathlib.Path(__file__).resolve().parents[1]

EARLIER = """snapshot 100000
entry stale-1
kind process
starter scheduler-1
state running
signal-interval 120
last-signal 96400
holds cache
owner done-run
entry done-run
kind run
starter supervisor-1
state finished
signal-interval 300
last-signal 90000
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
entry boss-run
kind run
starter supervisor-1
state running
signal-interval 30
last-signal 99995
holds none
owner none
"""

CLOSING = (
    "executes nothing, this report is a document and the tool "
    "performs no part of it\n"
)


def observe(earlier_text, later_text, entry_id="stale-1"):
    earlier = parse_snapshot(earlier_text, "earlier.txt")
    later = parse_snapshot(later_text, "later.txt")
    return build_observation(
        earlier, later, classify(earlier), entry_id
    )


def later_at(time, text=EARLIER):
    return text.replace("snapshot 100000", f"snapshot {time}", 1)


def without_stale_1(text):
    start = text.index("entry stale-1")
    end = text.index("entry done-run")
    return text[:start] + text[end:]


def test_the_watch_is_still_running_before_the_window():
    document = observe(EARLIER, later_at(100500))
    assert document == (
        "observation report for stale-1\n"
        "planned from snapshot 100000\n"
        "observed at snapshot 100500\n"
        "elapsed 500 seconds\n"
        "observation window 1200 seconds\n"
        "outcome watch still running\n"
        + CLOSING
    )


def test_the_quarantine_held_at_the_exact_window_boundary():
    document = observe(EARLIER, later_at(101200))
    assert document == (
        "observation report for stale-1\n"
        "planned from snapshot 100000\n"
        "observed at snapshot 101200\n"
        "elapsed 1200 seconds\n"
        "observation window 1200 seconds\n"
        "outcome quarantine held\n"
        "  evidence for the later snapshot records last signal "
        "96400 for the entry, the same last signal the earlier "
        "snapshot recorded\n"
        "  evidence for owner done-run is recorded finished\n"
        + CLOSING
    )


def test_the_quarantine_broke_on_a_later_signal_alone():
    later = later_at(101500).replace(
        "last-signal 96400", "last-signal 100800"
    )
    document = observe(EARLIER, later)
    assert "outcome quarantine broken\n" in document
    assert (
        "  evidence against the later snapshot records last "
        "signal 100800 for the entry, later than the last signal "
        "96400 the earlier snapshot recorded\n"
    ) in document
    assert (
        "  evidence for owner done-run is recorded finished\n"
        in document
    )


def test_the_quarantine_broke_on_an_owner_recorded_running():
    later = later_at(101500).replace("owner done-run", "owner boss-run")
    document = observe(EARLIER, later)
    assert "outcome quarantine broken\n" in document
    assert (
        "  evidence against owner boss-run is recorded running\n"
        in document
    )
    assert (
        "  evidence for the later snapshot records last signal "
        "96400 for the entry, the same last signal the earlier "
        "snapshot recorded\n"
    ) in document


def test_the_quarantine_held_when_the_entry_is_absent_later():
    document = observe(EARLIER, later_at(101500, without_stale_1(EARLIER)))
    assert "outcome quarantine held\n" in document
    assert (
        "  evidence for the entry is absent from the later "
        "snapshot, so no signal from it and no owner for it is "
        "recorded there\n"
    ) in document


def test_the_window_comes_from_the_earlier_interval_alone():
    later = later_at(100500).replace(
        "signal-interval 120", "signal-interval 10"
    )
    document = observe(EARLIER, later)
    assert "observation window 1200 seconds\n" in document
    assert "outcome watch still running\n" in document


def test_a_settled_later_entry_that_signalled_broke_the_watch():
    later = later_at(101500).replace(
        "last-signal 96400", "last-signal 100800"
    ).replace("state running\nsignal-interval 120", "state finished\nsignal-interval 120")
    later_snapshot = parse_snapshot(later, "later.txt")
    settled = classify_entry(
        later_snapshot, later_snapshot.by_id["stale-1"]
    )
    assert settled.classification == "settled"
    document = observe(EARLIER, later)
    assert "outcome quarantine broken\n" in document


def test_an_earlier_signal_value_later_is_no_sign_of_life():
    later = later_at(101500).replace(
        "last-signal 96400", "last-signal 90000"
    )
    document = observe(EARLIER, later)
    assert "outcome quarantine held\n" in document
    assert (
        "  evidence for the later snapshot records last signal "
        "90000 for the entry, which is not later than the last "
        "signal 96400 the earlier snapshot recorded\n"
    ) in document


def test_no_signal_recorded_later_is_no_sign_of_life():
    later = later_at(101500).replace(
        "last-signal 96400", "last-signal none"
    )
    document = observe(EARLIER, later)
    assert "outcome quarantine held\n" in document
    assert (
        "  evidence for the later snapshot records no signal "
        "for the entry\n"
    ) in document


def test_an_absent_or_unrecorded_owner_later_is_no_sign_of_life():
    absent = later_at(101500).replace("owner done-run", "owner ghost-7")
    document = observe(EARLIER, absent)
    assert "outcome quarantine held\n" in document
    assert (
        "  evidence for owner ghost-7 is absent from the snapshot\n"
        in document
    )
    unrecorded = later_at(101500).replace("owner done-run", "owner none")
    document = observe(EARLIER, unrecorded)
    assert "outcome quarantine held\n" in document
    assert (
        "  evidence for no owner reference is recorded for this "
        "entry\n"
    ) in document


def test_the_report_is_a_pure_function_of_the_two_texts():
    later = later_at(101500)
    assert observe(EARLIER, later) == observe(EARLIER, later)


def test_a_later_snapshot_not_later_is_refused():
    with pytest.raises(ObserveError) as caught:
        observe(EARLIER, EARLIER)
    message = str(caught.value)
    assert "stale-1" in message
    assert "100000" in message
    assert "not later" in message
    with pytest.raises(ObserveError):
        observe(EARLIER, later_at(99999))


def test_an_identifier_the_earlier_snapshot_lacks_is_refused():
    with pytest.raises(ObserveError) as caught:
        observe(EARLIER, later_at(101500), "ghost-9")
    message = str(caught.value)
    assert "ghost-9" in message
    assert "no entry in earlier.txt" in message


def test_an_entry_not_proven_in_the_earlier_snapshot_is_refused():
    with pytest.raises(ObserveError) as caught:
        observe(EARLIER, later_at(101500), "busy-1")
    assert "classified suspect" in str(caught.value)
    with pytest.raises(ObserveError) as caught:
        observe(EARLIER, later_at(101500), "done-run")
    assert "classified settled" in str(caught.value)


def write(tmp_path, name, text):
    path = tmp_path / name
    path.write_text(text, encoding="utf-8")
    return str(path)


def run_cli(*args):
    return subprocess.run(
        [sys.executable, "-m", "zombie_hunt", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def test_observe_prints_the_document_and_exits_zero(tmp_path):
    earlier = write(tmp_path, "early.txt", EARLIER)
    later = write(tmp_path, "later.txt", later_at(101200))
    result = run_cli("observe", earlier, later, "stale-1")
    assert result.returncode == 0
    assert result.stdout == observe(EARLIER, later_at(101200))
    assert result.stderr == ""


def test_an_observe_refusal_exits_two_with_one_error_line(tmp_path):
    earlier = write(tmp_path, "early.txt", EARLIER)
    later = write(tmp_path, "later.txt", later_at(101500))
    result = run_cli("observe", earlier, later, "busy-1")
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "classified suspect" in result.stderr
    assert "Traceback" not in result.stderr


def test_observe_accepts_no_fixtures_option(tmp_path):
    earlier = write(tmp_path, "early.txt", EARLIER)
    later = write(tmp_path, "later.txt", later_at(101500))
    result = run_cli(
        "observe", earlier, later, "stale-1", "--fixtures", "anywhere"
    )
    assert result.returncode == 2
    assert result.stdout == ""


def test_observe_modifies_none_of_its_inputs(tmp_path):
    earlier_path = tmp_path / "early.txt"
    later_path = tmp_path / "later.txt"
    earlier_path.write_text(EARLIER, encoding="utf-8")
    later_path.write_text(later_at(101200), encoding="utf-8")
    before = (earlier_path.read_bytes(), later_path.read_bytes())
    result = run_cli(
        "observe", str(earlier_path), str(later_path), "stale-1"
    )
    assert result.returncode == 0
    assert (earlier_path.read_bytes(), later_path.read_bytes()) == before
