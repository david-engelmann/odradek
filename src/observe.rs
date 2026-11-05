//! The observation report over the watch one quarantine plan sets.

use crate::errors::{Error, Result};
use crate::plan::OBSERVATION_MULTIPLE;
use crate::pytext::PyStr;
use crate::scan::{CONTRADICTED, PROVEN, Verdict, classify_entry};
use crate::snapshot::{Entry, Snapshot};

/// Write the observation report for one plan or refuse.
///
/// The report reads the outcome of the watch the plan for one
/// proven zombie set. The window is recomputed from the
/// earlier entry alone, exactly as the plan computed it, and
/// the verdict reads the recorded signal and owner facts of
/// the later snapshot and nothing else, never the
/// classification the scan gives the entry there. The earlier
/// entry of a proven zombie always carries a recorded last
/// signal, because its proof satisfied the signal rule, and
/// this reading relies on that rather than refusing its
/// absence. No judgment source is consulted and nothing is
/// performed.
pub fn build_observation(
    earlier: &Snapshot,
    later: &Snapshot,
    verdicts: &[Verdict],
    entry_id: &PyStr,
) -> Result<String> {
    let Some(entry) = earlier.entry(entry_id) else {
        return Err(Error::Observe(format!(
            "an observation for {entry_id} is refused, no entry in {} carries that identifier, and an observation is written only for a proven zombie in the earlier snapshot",
            earlier.source
        )));
    };
    let verdict = verdicts
        .iter()
        .find(|verdict| verdict.entry.id == entry.id)
        .expect("every entry carries a verdict");
    if verdict.classification != PROVEN {
        return Err(Error::Observe(format!(
            "an observation for {entry_id} is refused, the entry is classified {} in the earlier snapshot, and an observation is written only for a proven zombie",
            verdict.classification
        )));
    }
    if later.time <= earlier.time {
        return Err(Error::Observe(format!(
            "an observation for {entry_id} is refused, the later snapshot is recorded at {}, which is not later than the earlier snapshot time {}, and an observation reads a snapshot recorded after the one the plan was written from",
            later.time, earlier.time
        )));
    }
    let window = entry.signal_interval.mul_small(OBSERVATION_MULTIPLE);
    let elapsed = later.time.sub(&earlier.time);
    let mut lines = vec![
        format!("observation report for {}", entry.id),
        format!("planned from snapshot {}", earlier.time),
        format!("observed at snapshot {}", later.time),
        format!("elapsed {elapsed} seconds"),
        format!("observation window {window} seconds"),
    ];
    if elapsed < window {
        lines.push("outcome watch still running".to_string());
    } else {
        let (outcome, evidence) = verdict_of(earlier, later, entry);
        lines.push(outcome);
        lines.extend(evidence);
    }
    lines.push(
        "executes nothing, this report is a document and the tool performs no part of it"
            .to_string(),
    );
    Ok(lines.join("\n") + "\n")
}

/// The held or broken verdict and its evidence lines.
fn verdict_of(_earlier: &Snapshot, later: &Snapshot, entry: &Entry) -> (String, Vec<String>) {
    let Some(later_entry) = later.entry(&entry.id) else {
        return (
            "outcome quarantine held".to_string(),
            vec![
                "  evidence for the entry is absent from the later snapshot, so no signal from it and no owner for it is recorded there"
                    .to_string(),
            ],
        );
    };
    let mut facts = vec![signal_fact(entry, later_entry)];
    let owner = classify_entry(later, later_entry).owner;
    let owner_broke = owner.status == CONTRADICTED;
    let label = if owner_broke {
        "evidence against"
    } else {
        "evidence for"
    };
    facts.push((owner_broke, format!("{label} {}", owner.fact)));
    let broke = facts.iter().any(|(flag, _)| *flag);
    let outcome = if broke {
        "outcome quarantine broken"
    } else {
        "outcome quarantine held"
    };
    (
        outcome.to_string(),
        facts.iter().map(|(_, line)| format!("  {line}")).collect(),
    )
}

/// The signal fact of the watch, in the evidence line style.
fn signal_fact(entry: &Entry, later_entry: &Entry) -> (bool, String) {
    let recorded = entry
        .last_signal
        .as_ref()
        .expect("a proven zombie carries a recorded last signal");
    let seen = &later_entry.last_signal;
    if let Some(seen) = seen
        && seen > recorded
    {
        return (
            true,
            format!(
                "evidence against the later snapshot records last signal {seen} for the entry, later than the last signal {recorded} the earlier snapshot recorded"
            ),
        );
    }
    let Some(seen) = seen else {
        return (
            false,
            "evidence for the later snapshot records no signal for the entry".to_string(),
        );
    };
    if seen == recorded {
        return (
            false,
            format!(
                "evidence for the later snapshot records last signal {seen} for the entry, the same last signal the earlier snapshot recorded"
            ),
        );
    }
    (
        false,
        format!(
            "evidence for the later snapshot records last signal {seen} for the entry, which is not later than the last signal {recorded} the earlier snapshot recorded"
        ),
    )
}
