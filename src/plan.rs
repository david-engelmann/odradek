//! The reversible quarantine plan for one proven zombie.

use crate::errors::{Error, Result};
use crate::judgment::{JudgmentSource, check_answer, confidence_text, kill_call_request, trusted};
use crate::pytext::PyStr;
use crate::scan::{PROVEN, Verdict, check_computed, evidence_lines, holds_text};
use crate::snapshot::Snapshot;

pub const OBSERVATION_MULTIPLE: u32 = 10;

/// The two verdict lines a trusted kill call answer records.
///
/// The kill call is asked here and nowhere else, about the
/// proven zombie the plan names and after the plan refusals
/// have had their say. No judgment, which is an abstention,
/// an answer under the threshold, an answer outside the
/// recorded shape, or no source at all, leaves the plan
/// exactly as Round 1 wrote it. A trusted quarantine answer
/// advances the plan for operator approval. A trusted watch
/// or spare answer is recorded with the plan unadvanced.
/// Either way the plan stays a document and the tool
/// performs no part of it.
fn verdict_lines(verdict: &Verdict, source: &dyn JudgmentSource) -> Vec<String> {
    let answer = check_answer(source.judge(&kill_call_request(verdict)));
    if !trusted(answer.as_ref()) {
        return Vec::new();
    }
    let answer = answer.expect("a trusted answer stands");
    let label = answer
        .get("choice")
        .and_then(|value| value.as_str())
        .expect("a trusted answer names its choice")
        .to_display();
    let mut lines = vec![format!(
        "kill call verdict {label} with confidence {}",
        confidence_text(&answer)
    )];
    if label == "quarantine" {
        lines.push(
            "verdict advances this plan, the plan is ready for an operator to approve, and the approval itself stays an operator decision the tool never takes"
                .to_string(),
        );
    } else {
        lines.push(
            "verdict leaves this plan unadvanced, no action follows from this answer, and the plan stands as written for an operator to read"
                .to_string(),
        );
    }
    lines
}

/// Write the quarantine plan for one proven zombie or refuse.
pub fn build_plan(
    snapshot: &Snapshot,
    verdicts: &[Verdict],
    entry_id: &PyStr,
    source: Option<&dyn JudgmentSource>,
) -> Result<String> {
    let Some(entry) = snapshot.entry(entry_id) else {
        return Err(Error::Plan(format!(
            "a plan for {entry_id} is refused, no entry in {} carries that identifier, and a plan is written only for a proven zombie in the snapshot",
            snapshot.source
        )));
    };
    let verdict = verdicts
        .iter()
        .find(|verdict| verdict.entry.id == entry.id)
        .expect("every entry carries a verdict");
    if verdict.classification != PROVEN {
        return Err(Error::Plan(format!(
            "a plan for {entry_id} is refused, the entry is classified {}, and a plan is written only for a proven zombie",
            verdict.classification
        )));
    }
    let window = entry.signal_interval.mul_small(OBSERVATION_MULTIPLE);
    // The plan prints the window, and the print carries the
    // digit limit (ZH-31), checked here after the refusals,
    // where the oracle's formatting of the window line raises.
    check_computed(&window)?;
    let mut lines = vec![
        format!("quarantine plan for {}", entry.id),
        "classification proven zombie".to_string(),
        "proof".to_string(),
    ];
    for line in evidence_lines(verdict) {
        lines.push(format!("  {line}"));
    }
    lines.push(format!(
        "step 1 reversible, detach {} from {} so no new work reaches it",
        entry.id, entry.starter
    ));
    if !entry.holds.is_empty() {
        lines.push(format!(
            "step 2 reversible, fence the recorded holds, {}, so nothing else claims them while the entry is watched",
            holds_text(entry)
        ));
    } else {
        lines.push(
            "step 2 reversible, record that the entry holds nothing, so no resource needs fencing while the entry is watched"
                .to_string(),
        );
    }
    lines.push(format!(
        "step 3 reversible, hold the quarantine for the observation window and watch for any signal from {}",
        entry.id
    ));
    lines.push(format!("observation window {window} seconds"));
    lines.push(format!(
        "escalation, if {} shows no signal for the whole observation window and no owner comes forward, a kill proposal may be drafted in a later round, and this plan never escalates itself",
        entry.id
    ));
    if let Some(source) = source {
        lines.extend(verdict_lines(verdict, source));
    }
    lines.push(
        "executes nothing, this plan is a document and the tool performs no part of it".to_string(),
    );
    Ok(lines.join("\n") + "\n")
}
