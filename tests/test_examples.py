"""The committed example session regenerates byte for byte."""

import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "examples"


def run_cli(*args):
    return subprocess.run(
        [sys.executable, "-m", "zombie_hunt", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def test_the_example_scan_regenerates_byte_for_byte():
    result = run_cli("scan", str(EXAMPLES / "snapshot.txt"))
    assert result.returncode == 0
    assert result.stdout == (EXAMPLES / "scan.txt").read_text(
        encoding="utf-8"
    )


def test_the_example_plan_regenerates_byte_for_byte():
    result = run_cli("plan", str(EXAMPLES / "snapshot.txt"), "stale-1")
    assert result.returncode == 0
    assert result.stdout == (EXAMPLES / "plan.txt").read_text(
        encoding="utf-8"
    )


def test_the_example_observation_regenerates_byte_for_byte():
    result = run_cli(
        "observe",
        str(EXAMPLES / "snapshot.txt"),
        str(EXAMPLES / "later-snapshot.txt"),
        "stale-1",
    )
    assert result.returncode == 0
    assert result.stdout == (EXAMPLES / "observation.txt").read_text(
        encoding="utf-8"
    )


def test_the_example_session_shows_every_classification():
    scan = (EXAMPLES / "scan.txt").read_text(encoding="utf-8")
    assert "counts live 2 suspect 3 proven 2 settled 1" in scan
    assert "zombie stale-1" in scan
    assert "suspect quiet-1" in scan
