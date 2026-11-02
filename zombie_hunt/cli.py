"""The command line tool. Reads a recorded snapshot, prints documents."""

import argparse
import sys
from pathlib import Path

from .errors import ZombieHuntError
from .judgment import FixturePlayer
from .plan import build_plan
from .scan import classify, format_scan
from .snapshot import parse_snapshot


def _read(path):
    try:
        data = Path(path).read_bytes()
    except OSError as exc:
        raise ZombieHuntError(
            f"the snapshot at {path} cannot be read, {exc.strerror}"
        ) from exc
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ZombieHuntError(
            f"the snapshot at {path} is not readable as UTF-8 text"
        ) from exc


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="zombie_hunt",
        description=(
            "Reads one recorded cluster snapshot and prints the "
            "scan or a quarantine plan. Executes nothing."
        ),
    )
    sub = parser.add_subparsers(dest="command", required=True)
    scan_parser = sub.add_parser("scan")
    scan_parser.add_argument("snapshot")
    plan_parser = sub.add_parser("plan")
    plan_parser.add_argument("snapshot")
    plan_parser.add_argument("entry_id")
    plan_parser.add_argument("--fixtures")
    args = parser.parse_args(argv)
    try:
        snapshot = parse_snapshot(_read(args.snapshot), args.snapshot)
        verdicts = classify(snapshot)
        if args.command == "scan":
            document = format_scan(snapshot, verdicts)
        else:
            source = None
            if args.fixtures is not None:
                source = FixturePlayer(args.fixtures)
            document = build_plan(
                snapshot, verdicts, args.entry_id, source
            )
    except ZombieHuntError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    print(document, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
