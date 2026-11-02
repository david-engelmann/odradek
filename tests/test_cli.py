"""ZH-11 and ZH-12. The command line tool and its discipline."""

import pathlib
import subprocess
import sys

from tests.test_scan import EXPECTED_SCAN, SNAPSHOT

ROOT = pathlib.Path(__file__).resolve().parents[1]


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


def test_scan_prints_the_document_and_exits_zero(tmp_path):
    snap = write(tmp_path, "snap.txt", SNAPSHOT)
    result = run_cli("scan", snap)
    assert result.returncode == 0
    assert result.stdout == EXPECTED_SCAN
    assert result.stderr == ""


def test_plan_prints_the_document_and_exits_zero(tmp_path):
    snap = write(tmp_path, "snap.txt", SNAPSHOT)
    result = run_cli("plan", snap, "stale-1")
    assert result.returncode == 0
    assert result.stdout.startswith("quarantine plan for stale-1\n")
    assert result.stderr == ""


def test_a_malformed_snapshot_exits_two_with_one_error_line(tmp_path):
    snap = write(tmp_path, "snap.txt", "snapshot 100000\nentry a\nkind daemon\n")
    result = run_cli("scan", snap)
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "Traceback" not in result.stderr


def test_a_plan_refusal_exits_two_with_one_error_line(tmp_path):
    snap = write(tmp_path, "snap.txt", SNAPSHOT)
    result = run_cli("plan", snap, "busy-1")
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1
    assert "classified suspect" in result.stderr
    assert "Traceback" not in result.stderr


def test_a_missing_file_exits_two_with_one_error_line(tmp_path):
    result = run_cli("scan", str(tmp_path / "absent.txt"))
    assert result.returncode == 2
    assert result.stdout == ""
    assert len(result.stderr.splitlines()) == 1


def test_a_non_utf8_snapshot_exits_two_with_one_error_line(tmp_path):
    path = tmp_path / "snap.txt"
    path.write_bytes(b"snapshot 100000\nentry a\xff\n")
    result = run_cli("scan", str(path))
    assert result.returncode == 2
    assert len(result.stderr.splitlines()) == 1


def test_the_commands_modify_none_of_their_inputs(tmp_path):
    snap_path = tmp_path / "snap.txt"
    snap_path.write_text(SNAPSHOT, encoding="utf-8")
    before = snap_path.read_bytes()
    assert run_cli("scan", str(snap_path)).returncode == 0
    assert run_cli("plan", str(snap_path), "stale-1").returncode == 0
    assert snap_path.read_bytes() == before
