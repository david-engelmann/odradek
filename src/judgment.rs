//! Recorded kill call judgments, played back from fixtures.
//!
//! The roadmap gives zombie-hunt one decision point, the kill
//! call. When the proof pack for a proven zombie is complete,
//! a judgment answers one question over the recorded evidence,
//! does this workload serve nobody, with a choice over the
//! labels quarantine, watch, and spare. The answer is advisory.
//! A trusted quarantine answer advances the plan for operator
//! approval, a watch or spare answer is recorded on the plan
//! unadvanced, and no answer of any kind classifies an entry,
//! performs a step, or approves anything.
//!
//! A judgment is recorded as a fixture that pairs one kill
//! call request with one response in the shape a decision
//! model returns. A choice response carries the chosen label,
//! a confidence, and a probability for every label. An
//! abstention response carries none of those fields. The
//! fixture player answers a request only from a recorded
//! fixture whose request matches it exactly, and abstains
//! where no fixture matches. Everything here is offline. The
//! player reads files from one directory and nothing else.
//!
//! The JSON documents keep this crate's own value model and
//! writer, because the reader's ceilings and the canonical
//! match key's byte form are contracted (ZH-30 and ZH-32) and
//! a stock JSON library reproduces neither.

use std::collections::HashMap;
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::errors::{Error, Result};
use crate::paths::py_path_folded;
use crate::pyjson::{LoadsError, Value, dumps_canonical, loads, py_value_repr};
use crate::pytext::{PyStr, py_float_repr, py_repr};
use crate::scan::Verdict;

pub const CONFIDENCE_THRESHOLD: f64 = 0.6;

pub const JUDGMENT_POINT: &str = "kill-call";

pub const KILL_LABELS: [&str; 3] = ["quarantine", "watch", "spare"];
const RULE_FIELDS: [&str; 3] = ["signal", "owner", "in-flight"];
const RULE_OUTCOMES: [&str; 3] = ["satisfied", "contradicted", "unknown"];
const REQUEST_FIELDS: [&str; 4] = ["entry", "signal", "owner", "in-flight"];
const RULE_REQUEST_FIELDS: [&str; 2] = ["outcome", "evidence"];
const ENVELOPE_FIELDS: [&str; 4] = ["synthetic", "judgment_point", "request", "response"];
const SUM_TOLERANCE: f64 = 0.001;

/// A judgment source, answering kill call requests with no
/// answer where it abstains.
pub trait JudgmentSource {
    /// The response for one request, or None to abstain.
    fn judge(&self, request: &Value) -> Option<Value>;
}

fn fail(source: &str, defect: &str) -> Error {
    Error::Judgment(format!("{source} {defect}"))
}

fn fields_for_kind(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "abstention" => Some(&["kind"]),
        "choice" => Some(&["kind", "choice", "confidence", "probabilities"]),
        _ => None,
    }
}

/// The first of the extra fields, ordered by their printed
/// form, as the oracle's validation orders them.
fn first_extra<'a>(pairs: &'a [(PyStr, Value)], allowed: &[&str]) -> Option<&'a PyStr> {
    let mut extras: Vec<&PyStr> = pairs
        .iter()
        .map(|(key, _)| key)
        .filter(|key| !allowed.iter().any(|known| *key == known))
        .collect();
    extras.sort_by_key(|key| py_repr(key));
    extras.into_iter().next()
}

fn field_set_matches(pairs: &[(PyStr, Value)], expected: &[&str]) -> bool {
    pairs.len() == expected.len()
        && expected
            .iter()
            .all(|field| pairs.iter().any(|(key, _)| key == field))
}

fn check_unit_interval(source: &str, field: &str, value: Option<&Value>) -> Result<()> {
    let accepted = match value {
        Some(held) if held.is_number() => {
            matches!(held.as_f64(), Some(number) if (0.0..=1.0).contains(&number))
        }
        _ => false,
    };
    if !accepted {
        return Err(fail(
            source,
            &format!("field '{field}' is not a number between 0 and 1"),
        ));
    }
    Ok(())
}

fn validate_request(source: &str, request: &Value) -> Result<()> {
    let Value::Object(pairs) = request else {
        return Err(fail(
            source,
            "request does not carry exactly the fields entry, signal, owner, and in-flight",
        ));
    };
    if !field_set_matches(pairs, &REQUEST_FIELDS) {
        return Err(fail(
            source,
            "request does not carry exactly the fields entry, signal, owner, and in-flight",
        ));
    }
    match request.get("entry") {
        Some(Value::Str(text)) if !text.is_empty() => {}
        _ => {
            return Err(fail(
                source,
                "request field 'entry' is not a non-empty string",
            ));
        }
    }
    for name in RULE_FIELDS {
        let rule = request.get(name);
        let valid_shape = matches!(rule, Some(Value::Object(rule_pairs)) if field_set_matches(rule_pairs, &RULE_REQUEST_FIELDS));
        if !valid_shape {
            return Err(fail(
                source,
                &format!(
                    "request field '{name}' does not carry exactly the fields outcome and evidence"
                ),
            ));
        }
        let rule = rule.expect("a rule of the checked shape");
        let outcome_ok = matches!(rule.get("outcome"), Some(Value::Str(text)) if RULE_OUTCOMES.iter().any(|known| text == known));
        if !outcome_ok {
            return Err(fail(
                source,
                &format!(
                    "request field '{name}' names an outcome that is not satisfied, contradicted, or unknown"
                ),
            ));
        }
        match rule.get("evidence") {
            Some(Value::Str(text)) if !text.is_empty() => {}
            _ => {
                return Err(fail(
                    source,
                    &format!("request field '{name}' evidence is not a non-empty string"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_response(source: &str, response: &Value) -> Result<()> {
    let Value::Object(pairs) = response else {
        return Err(fail(source, "response is not an object"));
    };
    let kind = response.get("kind");
    if let Some(Value::Str(kind_text)) = kind
        && let Some(allowed) = fields_for_kind(&kind_text.to_display())
        && let Some(extra) = first_extra(pairs, allowed)
    {
        return Err(fail(
            source,
            &format!(
                "response carries the field {}, which a {} response does not carry",
                py_repr(extra),
                kind_text
            ),
        ));
    }
    if matches!(kind, Some(Value::Str(text)) if text == &"abstention") {
        return Ok(());
    }
    if !matches!(kind, Some(Value::Str(text)) if text == &"choice") {
        let shown = kind
            .map(py_value_repr)
            .unwrap_or_else(|| "None".to_string());
        return Err(fail(
            source,
            &format!(
                "response kind {shown} does not answer the kill call, which takes a choice or an abstention"
            ),
        ));
    }
    check_unit_interval(source, "confidence", response.get("confidence"))?;
    let choice_ok = matches!(response.get("choice"), Some(Value::Str(text)) if KILL_LABELS.iter().any(|label| text == label));
    if !choice_ok {
        return Err(fail(
            source,
            "response field 'choice' is not one of the labels quarantine, watch, and spare",
        ));
    }
    let probabilities = response.get("probabilities");
    let Some(Value::Object(probability_pairs)) = probabilities else {
        return Err(fail(
            source,
            "response field 'probabilities' is not an object",
        ));
    };
    if !field_set_matches(probability_pairs, &KILL_LABELS) {
        return Err(fail(
            source,
            "response probabilities do not cover exactly the labels quarantine, watch, and spare",
        ));
    }
    let mut total = 0.0f64;
    for label in KILL_LABELS {
        let value = probabilities.expect("a checked object").get(label);
        check_unit_interval(source, &format!("probabilities.{label}"), value)?;
        total += value
            .and_then(|held| held.as_f64())
            .expect("a checked number");
    }
    if (total - 1.0).abs() > SUM_TOLERANCE {
        return Err(fail(source, "response probabilities do not add up to 1"));
    }
    Ok(())
}

/// One recorded fixture pair, checked against the recorded
/// shape. Raises a judgment error naming the source and the
/// defect when the pair is outside the shape.
pub fn validate_fixture(data: &Value, source: &str) -> Result<()> {
    let Value::Object(pairs) = data else {
        return Err(fail(source, "is not an object"));
    };
    if let Some(extra) = first_extra(pairs, &ENVELOPE_FIELDS) {
        return Err(fail(
            source,
            &format!(
                "carries the field {}, which the fixture envelope does not carry",
                py_repr(extra)
            ),
        ));
    }
    if !matches!(data.get("synthetic"), Some(Value::Bool(true))) {
        return Err(fail(source, "is not marked synthetic"));
    }
    let point_ok =
        matches!(data.get("judgment_point"), Some(Value::Str(text)) if text == &JUDGMENT_POINT);
    if !point_ok {
        return Err(fail(
            source,
            "field 'judgment_point' is not the kill-call point",
        ));
    }
    let request = data.get("request");
    if !matches!(request, Some(Value::Object(_))) {
        return Err(fail(source, "field 'request' is not an object"));
    }
    validate_request(source, request.expect("a request object"))?;
    if !data.has("response") {
        return Err(fail(source, "carries no response"));
    }
    validate_response(source, data.get("response").expect("a response"))?;
    Ok(())
}

/// The canonical match key of one request, the JSON dump of
/// the point and request object with keys sorted and the
/// oracle's default separators and ASCII escaping.
fn request_key(request: &Value) -> String {
    let envelope = Value::Object(vec![
        (
            PyStr::from_str("point"),
            Value::Str(PyStr::from_str(JUDGMENT_POINT)),
        ),
        (PyStr::from_str("request"), request.clone()),
    ]);
    let mut out = String::new();
    dumps_canonical(&envelope, &mut out);
    out
}

/// Read and validate one fixture file. The read opens the
/// folded joined path, which the read messages also name,
/// and the name is the bare file name the validation source
/// names.
pub fn load_fixture_file(path_bytes: &[u8], name: &PyStr) -> Result<Value> {
    let folded = py_path_folded(path_bytes);
    let display = PyStr::from_surrogateescape(&folded);
    let bytes = std::fs::read(OsStr::from_bytes(&folded))
        .map_err(|_| Error::Judgment(format!("fixture file cannot be read, {display}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::Judgment(format!("fixture file is not valid UTF-8, {display}")))?;
    let data = loads(&PyStr::from_str(&text)).map_err(|error| match error {
        LoadsError::Invalid => {
            Error::Judgment(format!("fixture file is not valid JSON, {display}"))
        }
        LoadsError::Uncaught(line) => Error::Uncaught(line),
    })?;
    validate_fixture(&data, &format!("fixture {name}"))?;
    Ok(data)
}

/// Answers kill call requests from the recorded fixtures.
///
/// Every JSON file in the directory holds one fixture pair.
/// Pairs are validated as they load, so a response that
/// changes shape fails here, loudly, and never reaches the
/// plan.
#[derive(Debug)]
pub struct FixturePlayer {
    answers: HashMap<String, Value>,
}

impl FixturePlayer {
    /// Load every fixture in one directory, named as written.
    pub fn load(directory_bytes: &[u8]) -> Result<FixturePlayer> {
        let directory_display = PyStr::from_surrogateescape(directory_bytes);
        // The oracle's path object folds the directory as
        // written before it opens it, and the fold reads an
        // empty path as the single dot, while a refusal still
        // names the path as written.
        let directory_folded = py_path_folded(directory_bytes);
        let directory = Path::new(OsStr::from_bytes(&directory_folded));
        if !directory.is_dir() {
            return Err(Error::Judgment(format!(
                "fixtures directory cannot be read, {directory_display}"
            )));
        }
        // The oracle's glob suppresses listing failures, so a
        // directory that stands but cannot be listed simply
        // yields no fixtures.
        let mut names: Vec<Vec<u8>> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_bytes = name.as_bytes().to_vec();
                if name_bytes.ends_with(b".json") {
                    names.push(name_bytes);
                }
            }
        }
        // The oracle sorts the joined paths, which share the
        // directory prefix, so the file names decide the order,
        // compared as the strings argument decoding produces.
        names.sort_by(|left, right| {
            PyStr::from_surrogateescape(left).cmp(&PyStr::from_surrogateescape(right))
        });
        let mut answers: HashMap<String, Value> = HashMap::new();
        for name_bytes in &names {
            let mut joined = directory_bytes.to_vec();
            if !joined.is_empty() {
                joined.push(b'/');
            }
            joined.extend_from_slice(name_bytes);
            let name = PyStr::from_surrogateescape(name_bytes);
            let pair = load_fixture_file(&joined, &name)?;
            let key = request_key(pair.get("request").expect("a validated request"));
            if answers.contains_key(&key) {
                return Err(Error::Judgment(format!(
                    "fixture {name} records the same request as an earlier fixture"
                )));
            }
            answers.insert(
                key,
                pair.get("response").expect("a validated response").clone(),
            );
        }
        Ok(FixturePlayer { answers })
    }
}

impl JudgmentSource for FixturePlayer {
    /// The recorded response for this request, or None to
    /// abstain.
    fn judge(&self, request: &Value) -> Option<Value> {
        self.answers.get(&request_key(request)).cloned()
    }
}

/// The answer itself when it is well formed for this point,
/// else None.
///
/// A judgment source that is not the fixture player can hand
/// back anything. An answer outside the recorded shape for
/// the kill call is treated as no judgment at all, never
/// trusted and never recorded, so a malformed answer cannot
/// crash the plan or reach a plan document.
pub fn check_answer(response: Option<Value>) -> Option<Value> {
    let response = response?;
    match validate_response("answer", &response) {
        Ok(()) => Some(response),
        Err(_) => None,
    }
}

/// Whether a kill call answer may stand as a verdict at all.
///
/// An abstention and an answer whose confidence falls below
/// the threshold in the contract both count as no judgment
/// at all.
pub fn trusted(response: Option<&Value>) -> bool {
    let Some(response) = response else {
        return false;
    };
    if !matches!(response, Value::Object(_)) {
        return false;
    }
    if !matches!(response.get("kind"), Some(Value::Str(text)) if text == &"choice") {
        return false;
    }
    match response.get("confidence") {
        Some(confidence) if confidence.is_number() => {
            matches!(confidence.as_f64(), Some(number) if number >= CONFIDENCE_THRESHOLD)
        }
        _ => false,
    }
}

/// The kill call request the plan asks about one proven
/// zombie.
///
/// The request is built from the verdict the scan computed
/// and from nothing else, naming the entry and carrying each
/// proof rule outcome with the fact that outcome rests on,
/// copied word for word from the verdict.
pub fn kill_call_request(verdict: &Verdict) -> Value {
    fn rule(status: &str, fact: &str) -> Value {
        Value::Object(vec![
            (
                PyStr::from_str("outcome"),
                Value::Str(PyStr::from_str(status)),
            ),
            (
                PyStr::from_str("evidence"),
                Value::Str(PyStr::from_str(fact)),
            ),
        ])
    }
    Value::Object(vec![
        (
            PyStr::from_str("entry"),
            Value::Str(verdict.entry.id.clone()),
        ),
        (
            PyStr::from_str("signal"),
            rule(verdict.signal.status, &verdict.signal.fact),
        ),
        (
            PyStr::from_str("owner"),
            rule(verdict.owner.status, &verdict.owner.fact),
        ),
        (
            PyStr::from_str("in-flight"),
            rule(verdict.in_flight.status, &verdict.in_flight.fact),
        ),
    ])
}

/// The confidence of a trusted answer in its printed form,
/// the shortest decimal form that round trips to the float
/// the value converts to.
pub fn confidence_text(answer: &Value) -> String {
    let number = answer
        .get("confidence")
        .and_then(|value| value.as_f64())
        .expect("a trusted answer carries a convertible confidence");
    py_float_repr(number)
}
