//! The recorded cluster snapshot format and its parser.
//!
//! The format is defined in terms of the oracle runtime's
//! text operations, so lines are split on the newline
//! character alone and stripped and field-split on its
//! whitespace table, both carried by the text module. Whole
//! numbers read through one helper whose pattern is the full
//! Unicode decimal digit class and whose conversion accepts
//! mixed scripts digit by digit and carries the 4300 digit
//! character limit of ZH-31.

use crate::bignum::BigNum;
use crate::errors::{Error, Result};
use crate::pytext::{PyStr, decimal_value, is_decimal, py_repr};

pub const KINDS: [&str; 2] = ["process", "run"];
pub const STATES: [&str; 2] = ["running", "finished"];
pub const FIELDS: [&str; 7] = [
    "kind",
    "starter",
    "state",
    "signal-interval",
    "last-signal",
    "holds",
    "owner",
];
pub const IN_FLIGHT: &str = "in-flight";

/// The most digit characters a whole number conversion
/// admits, counted over every digit character including
/// leading zeros (ZH-31).
pub const MAX_WHOLE_DIGITS: usize = 4300;

/// The line the port prints when a whole number conversion
/// passes the digit limit, its own one line standing where
/// the oracle printed a traceback.
pub const WHOLE_DIGIT_LINE: &str = "odradek: a snapshot whole number exceeded its digit limit";

/// One recorded entry of a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: PyStr,
    pub kind: PyStr,
    pub starter: PyStr,
    pub state: PyStr,
    pub signal_interval: BigNum,
    pub last_signal: Option<BigNum>,
    pub holds: Vec<PyStr>,
    pub owner: Option<PyStr>,
}

/// One recorded cluster snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub source: PyStr,
    pub time: BigNum,
    pub entries: Vec<Entry>,
}

impl Snapshot {
    /// The entry carrying one identifier, where one stands.
    pub fn entry(&self, id: &PyStr) -> Option<&Entry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }
}

fn malformed(source: &PyStr, defect: String) -> Error {
    Error::Snapshot(format!("{source} is malformed, {defect}"))
}

/// The meaningful lines of a snapshot text, the lines that
/// are neither blank nor comments after stripping.
fn meaningful_lines(text: &PyStr) -> Vec<PyStr> {
    let mut lines = Vec::new();
    for raw in text.split_point(0x0A) {
        let stripped = raw.strip();
        if stripped.is_empty() || stripped.starts_with_point(0x23) {
            continue;
        }
        lines.push(stripped);
    }
    lines
}

/// One whole number read through the one helper every whole
/// number in the format reads through.
fn whole(value: &PyStr, source: &PyStr, what: &str) -> Result<BigNum> {
    if value.is_empty() || !value.0.iter().all(|point| is_decimal(*point)) {
        return Err(malformed(
            source,
            format!(
                "the {what} value {} is not a whole number of seconds",
                py_repr(value)
            ),
        ));
    }
    if value.len() > MAX_WHOLE_DIGITS {
        return Err(Error::Uncaught(WHOLE_DIGIT_LINE.to_string()));
    }
    let values: Vec<u32> = value
        .0
        .iter()
        .map(|point| decimal_value(*point).expect("a decimal digit has a value"))
        .collect();
    Ok(BigNum::from_digit_values(&values))
}

/// The stored form of one field value, converted and checked.
enum FieldValue {
    Text(PyStr),
    Interval(BigNum),
    Signal(Option<BigNum>),
    Holds(Vec<PyStr>),
    Owner(Option<PyStr>),
}

fn field_value(
    name: &str,
    value: &PyStr,
    source: &PyStr,
    entry_id: &PyStr,
    snapshot_time: &BigNum,
) -> Result<FieldValue> {
    let where_ = format!("the entry {entry_id}");
    match name {
        "kind" => {
            if !KINDS.iter().any(|kind| value == kind) {
                return Err(malformed(
                    source,
                    format!(
                        "{where_} names the kind {}, which is neither process nor run",
                        py_repr(value)
                    ),
                ));
            }
            Ok(FieldValue::Text(value.clone()))
        }
        "starter" => Ok(FieldValue::Text(value.clone())),
        "state" => {
            if !STATES.iter().any(|state| value == state) {
                return Err(malformed(
                    source,
                    format!(
                        "{where_} names the state {}, which is neither running nor finished",
                        py_repr(value)
                    ),
                ));
            }
            Ok(FieldValue::Text(value.clone()))
        }
        "signal-interval" => {
            let interval = whole(value, source, &format!("signal interval of {where_}"))?;
            if interval == BigNum::from_u64(0) {
                return Err(malformed(
                    source,
                    format!(
                        "{where_} names a signal interval of {}, which is not positive",
                        py_repr(value)
                    ),
                ));
            }
            Ok(FieldValue::Interval(interval))
        }
        "last-signal" => {
            if value == &"none" {
                return Ok(FieldValue::Signal(None));
            }
            let moment = whole(value, source, &format!("last signal of {where_}"))?;
            if moment > *snapshot_time {
                return Err(malformed(
                    source,
                    format!(
                        "{where_} names a last signal at {moment}, later than the snapshot time {snapshot_time}"
                    ),
                ));
            }
            Ok(FieldValue::Signal(Some(moment)))
        }
        "holds" => {
            if value == &"none" {
                return Ok(FieldValue::Holds(Vec::new()));
            }
            let tokens = value.split_point(0x2C);
            if tokens.iter().any(|token| token.is_empty())
                || tokens.iter().any(|token| token == &"none")
            {
                return Err(malformed(
                    source,
                    format!(
                        "{where_} names a holds list {} with an empty token or the word none inside the list",
                        py_repr(value)
                    ),
                ));
            }
            // ZH-20 reads the list as a set of tokens, so each
            // token is recorded once however many times the
            // list writes it, and every consumer downstream
            // sees the deduplicated form.
            let mut held: Vec<PyStr> = Vec::new();
            for token in tokens {
                if !held.contains(&token) {
                    held.push(token);
                }
            }
            Ok(FieldValue::Holds(held))
        }
        "owner" => {
            if value == &"none" {
                Ok(FieldValue::Owner(None))
            } else {
                Ok(FieldValue::Owner(Some(value.clone())))
            }
        }
        _ => unreachable!("a field name from the field table"),
    }
}

/// The entry a finished block names, or the refusal for the
/// first field it carries no line for.
fn finish_entry(
    fields: &[(String, FieldValue)],
    entry_id: &PyStr,
    source: &PyStr,
) -> Result<Entry> {
    for name in FIELDS {
        if !fields.iter().any(|(held, _)| held == name) {
            return Err(malformed(
                source,
                format!("the entry {entry_id} carries no {name} line"),
            ));
        }
    }
    let mut entry = Entry {
        id: entry_id.clone(),
        kind: PyStr::default(),
        starter: PyStr::default(),
        state: PyStr::default(),
        signal_interval: BigNum::from_u64(0),
        last_signal: None,
        holds: Vec::new(),
        owner: None,
    };
    for (name, value) in fields {
        match (name.as_str(), value) {
            ("kind", FieldValue::Text(text)) => entry.kind = text.clone(),
            ("starter", FieldValue::Text(text)) => entry.starter = text.clone(),
            ("state", FieldValue::Text(text)) => entry.state = text.clone(),
            ("signal-interval", FieldValue::Interval(interval)) => {
                entry.signal_interval = interval.clone()
            }
            ("last-signal", FieldValue::Signal(signal)) => entry.last_signal = signal.clone(),
            ("holds", FieldValue::Holds(holds)) => entry.holds = holds.clone(),
            ("owner", FieldValue::Owner(owner)) => entry.owner = owner.clone(),
            _ => unreachable!("a stored field value matches its field"),
        }
    }
    Ok(entry)
}

/// Parse one recorded cluster snapshot or refuse it.
pub fn parse_snapshot(text: &PyStr, source: &PyStr) -> Result<Snapshot> {
    let lines = meaningful_lines(text);
    if lines.is_empty() {
        return Err(malformed(source, "it names no snapshot header".to_string()));
    }
    let header = lines[0].split_whitespace();
    if header.len() != 2 || header[0] != "snapshot" {
        return Err(malformed(
            source,
            format!(
                "the first line {} is not a snapshot header naming the snapshot time",
                py_repr(&lines[0])
            ),
        ));
    }
    let snapshot_time = whole(&header[1], source, "snapshot time")?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen_ids: Vec<PyStr> = Vec::new();
    let mut entry_id: Option<PyStr> = None;
    let mut fields: Vec<(String, FieldValue)> = Vec::new();
    for line in &lines[1..] {
        let parts = line.split_whitespace();
        if parts[0] == "snapshot" {
            return Err(malformed(
                source,
                "the snapshot header appears more than once".to_string(),
            ));
        }
        if parts[0] == "entry" {
            if parts.len() != 2 {
                return Err(malformed(
                    source,
                    format!(
                        "the line {} does not open an entry with exactly one identifier",
                        py_repr(line)
                    ),
                ));
            }
            if let Some(open) = &entry_id {
                entries.push(finish_entry(&fields, open, source)?);
            }
            entry_id = Some(parts[1].clone());
            if seen_ids.contains(&parts[1]) {
                return Err(malformed(
                    source,
                    format!("the entry identifier {} is entered twice", parts[1]),
                ));
            }
            seen_ids.push(parts[1].clone());
            fields = Vec::new();
            continue;
        }
        let Some(open) = &entry_id else {
            return Err(malformed(
                source,
                format!("the line {} stands before any entry block", py_repr(line)),
            ));
        };
        let name = if parts.len() == 2 {
            FIELDS.iter().find(|field| parts[0] == **field)
        } else {
            None
        };
        let Some(name) = name else {
            return Err(malformed(
                source,
                format!(
                    "the line {} in the entry {open} matches no field form",
                    py_repr(line)
                ),
            ));
        };
        if fields.iter().any(|(held, _)| held == name) {
            return Err(malformed(
                source,
                format!("the entry {open} carries the {name} line twice"),
            ));
        }
        let value = field_value(name, &parts[1], source, open, &snapshot_time)?;
        fields.push((name.to_string(), value));
    }
    if let Some(open) = &entry_id {
        entries.push(finish_entry(&fields, open, source)?);
    }
    Ok(Snapshot {
        source: source.clone(),
        time: snapshot_time,
        entries,
    })
}
