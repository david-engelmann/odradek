"""The reversible quarantine plan for one proven zombie."""

from .errors import PlanError
from .judgment import check_answer, kill_call_request, trusted
from .scan import PROVEN, evidence_lines

OBSERVATION_MULTIPLE = 10


def _verdict_lines(verdict, source):
    """The two verdict lines a trusted kill call answer records.

    The kill call is asked here and nowhere else, about the proven
    zombie the plan names and after the plan refusals have had
    their say. No judgment, which is an abstention, an answer under
    the threshold, an answer outside the recorded shape, or no
    source at all, leaves the plan exactly as Round 1 wrote it. A
    trusted quarantine answer advances the plan for operator
    approval. A trusted watch or spare answer is recorded with the
    plan unadvanced. Either way the plan stays a document and the
    tool performs no part of it.
    """
    answer = check_answer(source.judge(kill_call_request(verdict)))
    if not trusted(answer):
        return []
    label = answer["choice"]
    lines = [
        "kill call verdict "
        + label
        + " with confidence "
        + repr(float(answer["confidence"]))
    ]
    if label == "quarantine":
        lines.append(
            "verdict advances this plan, the plan is ready for "
            "an operator to approve, and the approval itself "
            "stays an operator decision the tool never takes"
        )
    else:
        lines.append(
            "verdict leaves this plan unadvanced, no action "
            "follows from this answer, and the plan stands as "
            "written for an operator to read"
        )
    return lines


def build_plan(snapshot, verdicts, entry_id, source=None):
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
    if source is not None:
        lines.extend(_verdict_lines(verdict, source))
    lines.append(
        "executes nothing, this plan is a document and the "
        "tool performs no part of it"
    )
    return "\n".join(lines) + "\n"
