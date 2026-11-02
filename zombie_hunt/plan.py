"""The reversible quarantine plan for one proven zombie."""

from .errors import PlanError
from .scan import PROVEN, evidence_lines

OBSERVATION_MULTIPLE = 10


def build_plan(snapshot, verdicts, entry_id):
    """Write the quarantine plan for one proven zombie or refuse."""
    entry = snapshot.by_id.get(entry_id)
    if entry is None:
        raise PlanError(
            f"a plan for {entry_id} is refused, no entry in "
            f"{snapshot.source} carries that identifier, and a plan "
            "is written only for a proven zombie in the snapshot"
        )
    verdict = next(v for v in verdicts if v.entry is entry)
    if verdict.classification != PROVEN:
        raise PlanError(
            f"a plan for {entry_id} is refused, the entry is "
            f"classified {verdict.classification}, and a plan is "
            "written only for a proven zombie"
        )
    window = OBSERVATION_MULTIPLE * entry.signal_interval
    lines = [
        f"quarantine plan for {entry.id}",
        "classification proven zombie",
        "proof",
    ]
    for line in evidence_lines(verdict):
        lines.append(f"  {line}")
    lines.append(
        f"step 1 reversible, detach {entry.id} from "
        f"{entry.starter} so no new work reaches it"
    )
    if entry.holds:
        holds = ", ".join(sorted(entry.holds))
        lines.append(
            f"step 2 reversible, fence the recorded holds, "
            f"{holds}, so nothing else claims them while the "
            "entry is watched"
        )
    else:
        lines.append(
            "step 2 reversible, record that the entry holds "
            "nothing, so no resource needs fencing while the "
            "entry is watched"
        )
    lines.append(
        f"step 3 reversible, hold the quarantine for the "
        f"observation window and watch for any signal from "
        f"{entry.id}"
    )
    lines.append(f"observation window {window} seconds")
    lines.append(
        f"escalation, if {entry.id} shows no signal for the "
        "whole observation window and no owner comes forward, "
        "a kill proposal may be drafted in a later round, and "
        "this plan never escalates itself"
    )
    lines.append(
        "executes nothing, this plan is a document and the "
        "tool performs no part of it"
    )
    return "\n".join(lines) + "\n"
