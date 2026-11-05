//! The proof rules, the classification, and the scan document.

use crate::pytext::PyStr;
use crate::snapshot::{Entry, IN_FLIGHT, Snapshot};

pub const SATISFIED: &str = "satisfied";
pub const CONTRADICTED: &str = "contradicted";
pub const UNKNOWN: &str = "unknown";

pub const LIVE: &str = "live";
pub const SUSPECT: &str = "suspect";
pub const PROVEN: &str = "proven zombie";
pub const SETTLED: &str = "settled";

pub const PROOF_MULTIPLE: u32 = 3;

/// One proof rule's outcome for one entry, with the recorded
/// fact the outcome rests on.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleVerdict {
    pub status: &'static str,
    pub fact: String,
}

/// The classification of one entry with its three rule
/// verdicts.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict<'a> {
    pub entry: &'a Entry,
    pub classification: &'static str,
    pub signal: RuleVerdict,
    pub owner: RuleVerdict,
    pub in_flight: RuleVerdict,
}

impl Verdict<'_> {
    /// The three rule verdicts in the order the documents
    /// print them.
    pub fn rules(&self) -> [&RuleVerdict; 3] {
        [&self.signal, &self.owner, &self.in_flight]
    }
}

fn signal_rule(snapshot: &Snapshot, entry: &Entry) -> RuleVerdict {
    let threshold = entry.signal_interval.mul_small(PROOF_MULTIPLE);
    let Some(last_signal) = &entry.last_signal else {
        return RuleVerdict {
            status: UNKNOWN,
            fact: "no signal is recorded for this entry".to_string(),
        };
    };
    let age = snapshot.time.sub(last_signal);
    if age >= threshold {
        RuleVerdict {
            status: SATISFIED,
            fact: format!(
                "signal age {age} seconds reaches the proof threshold of {threshold} seconds"
            ),
        }
    } else {
        RuleVerdict {
            status: CONTRADICTED,
            fact: format!(
                "signal age {age} seconds is under the proof threshold of {threshold} seconds"
            ),
        }
    }
}

fn owner_rule(snapshot: &Snapshot, entry: &Entry) -> RuleVerdict {
    let Some(owner_id) = &entry.owner else {
        return RuleVerdict {
            status: UNKNOWN,
            fact: "no owner reference is recorded for this entry".to_string(),
        };
    };
    match snapshot.entry(owner_id) {
        None => RuleVerdict {
            status: SATISFIED,
            fact: format!("owner {owner_id} is absent from the snapshot"),
        },
        Some(owner) if owner.state == "finished" => RuleVerdict {
            status: SATISFIED,
            fact: format!("owner {owner_id} is recorded finished"),
        },
        Some(_) => RuleVerdict {
            status: CONTRADICTED,
            fact: format!("owner {owner_id} is recorded running"),
        },
    }
}

/// The holds tokens in the printed order, sorted as the
/// oracle's string order sorts them.
pub fn sorted_holds(entry: &Entry) -> Vec<PyStr> {
    let mut holds = entry.holds.clone();
    holds.sort();
    holds
}

/// The holds tokens in their printed joined form.
pub fn holds_text(entry: &Entry) -> String {
    let separator = PyStr::from_str(", ");
    PyStr::join(&sorted_holds(entry), &separator).to_display()
}

fn in_flight_rule(_snapshot: &Snapshot, entry: &Entry) -> RuleVerdict {
    if entry.holds.iter().any(|token| token == &IN_FLIGHT) {
        return RuleVerdict {
            status: CONTRADICTED,
            fact: format!("holds {} including the in-flight marker", holds_text(entry)),
        };
    }
    if entry.holds.is_empty() {
        return RuleVerdict {
            status: SATISFIED,
            fact: "the entry holds nothing and carries no in-flight marker".to_string(),
        };
    }
    RuleVerdict {
        status: SATISFIED,
        fact: format!("holds {} with no in-flight marker", holds_text(entry)),
    }
}

/// Classify one entry under the proof rules and nothing else.
pub fn classify_entry<'a>(snapshot: &'a Snapshot, entry: &'a Entry) -> Verdict<'a> {
    let signal = signal_rule(snapshot, entry);
    let owner = owner_rule(snapshot, entry);
    let in_flight = in_flight_rule(snapshot, entry);
    let live = match &entry.last_signal {
        Some(last_signal) => snapshot.time.sub(last_signal) < entry.signal_interval,
        None => false,
    };
    let all_satisfied = [&signal, &owner, &in_flight]
        .iter()
        .all(|rule| rule.status == SATISFIED);
    let classification = if entry.state == "finished" {
        SETTLED
    } else if live {
        LIVE
    } else if all_satisfied {
        PROVEN
    } else {
        SUSPECT
    };
    Verdict {
        entry,
        classification,
        signal,
        owner,
        in_flight,
    }
}

/// Classify every entry in the snapshot, in recorded order.
pub fn classify(snapshot: &Snapshot) -> Vec<Verdict<'_>> {
    snapshot
        .entries
        .iter()
        .map(|entry| classify_entry(snapshot, entry))
        .collect()
}

fn label(status: &str) -> &'static str {
    match status {
        SATISFIED => "evidence for",
        CONTRADICTED => "evidence against",
        _ => "missing",
    }
}

/// The three evidence lines of a verdict, one per proof rule.
pub fn evidence_lines(verdict: &Verdict) -> Vec<String> {
    verdict
        .rules()
        .iter()
        .map(|rule| format!("{} {}", label(rule.status), rule.fact))
        .collect()
}

/// Render the scan document as deterministic text.
pub fn format_scan(snapshot: &Snapshot, verdicts: &[Verdict]) -> String {
    let mut counts = [0usize; 4];
    for verdict in verdicts {
        match verdict.classification {
            LIVE => counts[0] += 1,
            SUSPECT => counts[1] += 1,
            PROVEN => counts[2] += 1,
            _ => counts[3] += 1,
        }
    }
    let mut lines = vec![
        format!("snapshot {}", snapshot.time),
        format!(
            "counts live {} suspect {} proven {} settled {}",
            counts[0], counts[1], counts[2], counts[3]
        ),
    ];
    let mut listed: Vec<&Verdict> = verdicts
        .iter()
        .filter(|verdict| verdict.classification == PROVEN || verdict.classification == SUSPECT)
        .collect();
    listed.sort_by(|left, right| {
        let rank = |verdict: &Verdict| {
            if verdict.classification == PROVEN {
                0
            } else {
                1
            }
        };
        rank(left)
            .cmp(&rank(right))
            .then_with(|| left.entry.id.cmp(&right.entry.id))
    });
    for verdict in listed {
        let word = if verdict.classification == PROVEN {
            "zombie"
        } else {
            "suspect"
        };
        lines.push(format!("{word} {}", verdict.entry.id));
        for line in evidence_lines(verdict) {
            lines.push(format!("  {line}"));
        }
    }
    lines.join("\n") + "\n"
}
