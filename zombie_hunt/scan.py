"""The proof rules, the classification, and the scan document."""

from dataclasses import dataclass

from .snapshot import IN_FLIGHT

SATISFIED = "satisfied"
CONTRADICTED = "contradicted"
UNKNOWN = "unknown"

LIVE = "live"
SUSPECT = "suspect"
PROVEN = "proven zombie"
SETTLED = "settled"

PROOF_MULTIPLE = 3


@dataclass(frozen=True)
class RuleVerdict:
    status: str
    fact: str


@dataclass(frozen=True)
class Verdict:
    entry: object
    classification: str
    signal: RuleVerdict
    owner: RuleVerdict
    in_flight: RuleVerdict

    @property
    def rules(self):
        return (self.signal, self.owner, self.in_flight)


def _signal_rule(snapshot, entry):
    threshold = PROOF_MULTIPLE * entry.signal_interval
    if entry.last_signal is None:
        return RuleVerdict(
            UNKNOWN, "no signal is recorded for this entry"
        )
    age = snapshot.time - entry.last_signal
    if age >= threshold:
        return RuleVerdict(
            SATISFIED,
            f"signal age {age} seconds reaches the proof threshold "
            f"of {threshold} seconds",
        )
    return RuleVerdict(
        CONTRADICTED,
        f"signal age {age} seconds is under the proof threshold "
        f"of {threshold} seconds",
    )


def _owner_rule(snapshot, entry):
    if entry.owner is None:
        return RuleVerdict(
            UNKNOWN, "no owner reference is recorded for this entry"
        )
    owner = snapshot.by_id.get(entry.owner)
    if owner is None:
        return RuleVerdict(
            SATISFIED,
            f"owner {entry.owner} is absent from the snapshot",
        )
    if owner.state == "finished":
        return RuleVerdict(
            SATISFIED,
            f"owner {entry.owner} is recorded finished",
        )
    return RuleVerdict(
        CONTRADICTED, f"owner {entry.owner} is recorded running"
    )


def _in_flight_rule(snapshot, entry):
    holds = ", ".join(sorted(entry.holds))
    if IN_FLIGHT in entry.holds:
        return RuleVerdict(
            CONTRADICTED,
            f"holds {holds} including the in-flight marker",
        )
    if not entry.holds:
        return RuleVerdict(
            SATISFIED,
            "the entry holds nothing and carries no in-flight marker",
        )
    return RuleVerdict(
        SATISFIED, f"holds {holds} with no in-flight marker"
    )


def classify_entry(snapshot, entry):
    """Classify one entry under the proof rules and nothing else."""
    signal = _signal_rule(snapshot, entry)
    owner = _owner_rule(snapshot, entry)
    in_flight = _in_flight_rule(snapshot, entry)
    if entry.state == "finished":
        classification = SETTLED
    elif (
        entry.last_signal is not None
        and snapshot.time - entry.last_signal < entry.signal_interval
    ):
        classification = LIVE
    elif all(
        rule.status == SATISFIED for rule in (signal, owner, in_flight)
    ):
        classification = PROVEN
    else:
        classification = SUSPECT
    return Verdict(entry, classification, signal, owner, in_flight)


def classify(snapshot):
    """Classify every entry in the snapshot, in recorded order."""
    return tuple(
        classify_entry(snapshot, entry) for entry in snapshot.entries
    )


_LABELS = {
    SATISFIED: "evidence for",
    CONTRADICTED: "evidence against",
    UNKNOWN: "missing",
}


def evidence_lines(verdict):
    """The three evidence lines of a verdict, one per proof rule."""
    return tuple(
        f"{_LABELS[rule.status]} {rule.fact}" for rule in verdict.rules
    )


def format_scan(snapshot, verdicts):
    """Render the scan document as deterministic text."""
    counts = {LIVE: 0, SUSPECT: 0, PROVEN: 0, SETTLED: 0}
    for verdict in verdicts:
        counts[verdict.classification] += 1
    lines = [
        f"snapshot {snapshot.time}",
        f"counts live {counts[LIVE]} suspect {counts[SUSPECT]} "
        f"proven {counts[PROVEN]} settled {counts[SETTLED]}",
    ]
    listed = [
        verdict
        for verdict in verdicts
        if verdict.classification in (PROVEN, SUSPECT)
    ]
    listed.sort(
        key=lambda verdict: (
            0 if verdict.classification == PROVEN else 1,
            verdict.entry.id,
        )
    )
    for verdict in listed:
        word = "zombie" if verdict.classification == PROVEN else "suspect"
        lines.append(f"{word} {verdict.entry.id}")
        for line in evidence_lines(verdict):
            lines.append(f"  {line}")
    return "\n".join(lines) + "\n"
