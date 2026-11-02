"""The recorded cluster snapshot format and its parser."""

import re
from dataclasses import dataclass

from .errors import SnapshotError

KINDS = ("process", "run")
STATES = ("running", "finished")
FIELDS = (
    "kind",
    "starter",
    "state",
    "signal-interval",
    "last-signal",
    "holds",
    "owner",
)
WHOLE = re.compile(r"^\d+$")
IN_FLIGHT = "in-flight"


@dataclass(frozen=True)
class Entry:
    id: str
    kind: str
    starter: str
    state: str
    signal_interval: int
    last_signal: int | None
    holds: tuple
    owner: str | None


@dataclass(frozen=True)
class Snapshot:
    source: str
    time: int
    entries: tuple
    by_id: dict


def _meaningful_lines(text):
    lines = []
    for raw in text.split("\n"):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        lines.append(stripped)
    return lines


def _whole(value, source, what):
    if not WHOLE.match(value):
        raise SnapshotError(
            f"{source} is malformed, the {what} value {value!r} is not "
            "a whole number of seconds"
        )
    return int(value)


def _finish_entry(fields, entry_id, source):
    missing = [name for name in FIELDS if name not in fields]
    if missing:
        raise SnapshotError(
            f"{source} is malformed, the entry {entry_id} carries no "
            f"{missing[0]} line"
        )
    return Entry(
        id=entry_id,
        kind=fields["kind"],
        starter=fields["starter"],
        state=fields["state"],
        signal_interval=fields["signal-interval"],
        last_signal=fields["last-signal"],
        holds=fields["holds"],
        owner=fields["owner"],
    )


def _field_value(name, value, source, entry_id, snapshot_time):
    where = f"the entry {entry_id}"
    if name == "kind":
        if value not in KINDS:
            raise SnapshotError(
                f"{source} is malformed, {where} names the kind "
                f"{value!r}, which is neither process nor run"
            )
        return value
    if name == "starter":
        return value
    if name == "state":
        if value not in STATES:
            raise SnapshotError(
                f"{source} is malformed, {where} names the state "
                f"{value!r}, which is neither running nor finished"
            )
        return value
    if name == "signal-interval":
        interval = _whole(value, source, f"signal interval of {where}")
        if interval <= 0:
            raise SnapshotError(
                f"{source} is malformed, {where} names a signal "
                f"interval of {value!r}, which is not positive"
            )
        return interval
    if name == "last-signal":
        if value == "none":
            return None
        moment = _whole(value, source, f"last signal of {where}")
        if moment > snapshot_time:
            raise SnapshotError(
                f"{source} is malformed, {where} names a last signal "
                f"at {moment}, later than the snapshot time "
                f"{snapshot_time}"
            )
        return moment
    if name == "holds":
        if value == "none":
            return ()
        tokens = tuple(value.split(","))
        if any(not token for token in tokens) or "none" in tokens:
            raise SnapshotError(
                f"{source} is malformed, {where} names a holds list "
                f"{value!r} with an empty token or the word none "
                "inside the list"
            )
        return tokens
    if name == "owner":
        if value == "none":
            return None
        return value
    raise AssertionError(name)


def parse_snapshot(text, source):
    """Parse one recorded cluster snapshot or raise SnapshotError."""
    lines = _meaningful_lines(text)
    if not lines:
        raise SnapshotError(
            f"{source} is malformed, it names no snapshot header"
        )
    header = lines[0].split()
    if len(header) != 2 or header[0] != "snapshot":
        raise SnapshotError(
            f"{source} is malformed, the first line {lines[0]!r} is "
            "not a snapshot header naming the snapshot time"
        )
    snapshot_time = _whole(header[1], source, "snapshot time")
    entries = []
    seen_ids = set()
    entry_id = None
    fields = None
    for line in lines[1:]:
        parts = line.split()
        if parts[0] == "snapshot":
            raise SnapshotError(
                f"{source} is malformed, the snapshot header appears "
                "more than once"
            )
        if parts[0] == "entry":
            if len(parts) != 2:
                raise SnapshotError(
                    f"{source} is malformed, the line {line!r} does "
                    "not open an entry with exactly one identifier"
                )
            if entry_id is not None:
                entries.append(_finish_entry(fields, entry_id, source))
            entry_id = parts[1]
            if entry_id in seen_ids:
                raise SnapshotError(
                    f"{source} is malformed, the entry identifier "
                    f"{entry_id} is entered twice"
                )
            seen_ids.add(entry_id)
            fields = {}
            continue
        if entry_id is None:
            raise SnapshotError(
                f"{source} is malformed, the line {line!r} stands "
                "before any entry block"
            )
        if len(parts) != 2 or parts[0] not in FIELDS:
            raise SnapshotError(
                f"{source} is malformed, the line {line!r} in the "
                f"entry {entry_id} matches no field form"
            )
        name, value = parts
        if name in fields:
            raise SnapshotError(
                f"{source} is malformed, the entry {entry_id} "
                f"carries the {name} line twice"
            )
        fields[name] = _field_value(
            name, value, source, entry_id, snapshot_time
        )
    if entry_id is not None:
        entries.append(_finish_entry(fields, entry_id, source))
    return Snapshot(
        source=source,
        time=snapshot_time,
        entries=tuple(entries),
        by_id={entry.id: entry for entry in entries},
    )
