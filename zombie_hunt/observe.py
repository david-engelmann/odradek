"""The observation report over the watch one quarantine plan sets."""

from .errors import ObserveError
from .plan import OBSERVATION_MULTIPLE
from .scan import CONTRADICTED, PROVEN, classify_entry


def build_observation(earlier, later, verdicts, entry_id):
    """Write the observation report for one plan or refuse.

    The report reads the outcome of the watch the plan for one
    proven zombie set. The window is recomputed from the earlier
    entry alone, exactly as the plan computed it, and the verdict
    reads the recorded signal and owner facts of the later
    snapshot and nothing else, never the classification the scan
    gives the entry there. The earlier entry of a proven zombie
    always carries a recorded last signal, because its proof
    satisfied the signal rule, and this reading relies on that
    rather than refusing its absence. No judgment source is
    consulted and nothing is performed.
    """
    entry = earlier.by_id.get(entry_id)
    if entry is None:
        raise ObserveError(
            f"an observation for {entry_id} is refused, no entry "
            f"in {earlier.source} carries that identifier, and an "
            "observation is written only for a proven zombie in "
            "the earlier snapshot"
        )
    verdict = next(v for v in verdicts if v.entry is entry)
    if verdict.classification != PROVEN:
        raise ObserveError(
            f"an observation for {entry_id} is refused, the entry "
            f"is classified {verdict.classification} in the "
            "earlier snapshot, and an observation is written only "
            "for a proven zombie"
        )
    if later.time <= earlier.time:
        raise ObserveError(
            f"an observation for {entry_id} is refused, the later "
            f"snapshot is recorded at {later.time}, which is not "
            f"later than the earlier snapshot time {earlier.time}, "
            "and an observation reads a snapshot recorded after "
            "the one the plan was written from"
        )
    window = OBSERVATION_MULTIPLE * entry.signal_interval
    elapsed = later.time - earlier.time
    lines = [
        f"observation report for {entry.id}",
        f"planned from snapshot {earlier.time}",
        f"observed at snapshot {later.time}",
        f"elapsed {elapsed} seconds",
        f"observation window {window} seconds",
    ]
    if elapsed < window:
        lines.append("outcome watch still running")
    else:
        outcome, evidence = _verdict(earlier, later, entry)
        lines.append(outcome)
        lines.extend(evidence)
    lines.append(
        "executes nothing, this report is a document and the "
        "tool performs no part of it"
    )
    return "\n".join(lines) + "\n"


def _verdict(earlier, later, entry):
    """The held or broken verdict and its evidence lines."""
    later_entry = later.by_id.get(entry.id)
    if later_entry is None:
        return "outcome quarantine held", [
            "  evidence for the entry is absent from the later "
            "snapshot, so no signal from it and no owner for it "
            "is recorded there"
        ]
    facts = [_signal_fact(entry, later_entry)]
    owner = classify_entry(later, later_entry).owner
    owner_broke = owner.status == CONTRADICTED
    label = "evidence against" if owner_broke else "evidence for"
    facts.append((owner_broke, f"{label} {owner.fact}"))
    broke = any(flag for flag, _ in facts)
    outcome = (
        "outcome quarantine broken" if broke else "outcome quarantine held"
    )
    return outcome, [f"  {line}" for _, line in facts]


def _signal_fact(entry, later_entry):
    """The signal fact of the watch, in the evidence line style."""
    recorded = entry.last_signal
    seen = later_entry.last_signal
    if seen is not None and seen > recorded:
        return (
            True,
            "evidence against the later snapshot records last "
            f"signal {seen} for the entry, later than the last "
            f"signal {recorded} the earlier snapshot recorded",
        )
    if seen is None:
        return (
            False,
            "evidence for the later snapshot records no signal "
            "for the entry",
        )
    if seen == recorded:
        return (
            False,
            "evidence for the later snapshot records last signal "
            f"{seen} for the entry, the same last signal the "
            "earlier snapshot recorded",
        )
    return (
        False,
        "evidence for the later snapshot records last signal "
        f"{seen} for the entry, which is not later than the "
        f"last signal {recorded} the earlier snapshot recorded",
    )
