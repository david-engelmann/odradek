//! ZH-14 to ZH-19. The kill call, its fixtures, and its plan rule.

mod common;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use zombie_hunt::errors::Error;
use zombie_hunt::judgment::{
    FixturePlayer, JudgmentSource, check_answer, kill_call_request, trusted, validate_fixture,
};
use zombie_hunt::plan::build_plan;
use zombie_hunt::pyjson::{Value, dumps_canonical, loads};
use zombie_hunt::pytext::PyStr;
use zombie_hunt::scan::classify;
use zombie_hunt::snapshot::parse_snapshot;

use common::{
    ADVANCED_PLAN, EXPECTED_PLAN, SNAPSHOT_B, SNAPSHOT_TEXT, STALE_1_REQUEST_CANONICAL, WATCH_PLAN,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn v(text: &str) -> Value {
    loads(&PyStr::from_str(text)).expect("json parses")
}

fn canonical(value: &Value) -> String {
    let mut out = String::new();
    dumps_canonical(value, &mut out);
    out
}

const REQUEST_JSON: &str = "{\"entry\": \"stale-1\", \"signal\": {\"outcome\": \"satisfied\", \"evidence\": \"signal age 3600 seconds reaches the proof threshold of 360 seconds\"}, \"owner\": {\"outcome\": \"satisfied\", \"evidence\": \"owner done-run is recorded finished\"}, \"in-flight\": {\"outcome\": \"satisfied\", \"evidence\": \"holds cache with no in-flight marker\"}}";

const QUARANTINE_JSON: &str = "{\"kind\": \"choice\", \"choice\": \"quarantine\", \"confidence\": 0.9, \"probabilities\": {\"quarantine\": 0.9, \"watch\": 0.07, \"spare\": 0.03}}";

fn pair(request: &str, response: &str) -> String {
    format!(
        "{{\"synthetic\": true, \"judgment_point\": \"kill-call\", \"request\": {request}, \"response\": {response}}}"
    )
}

fn verdict_and_snap(text: &str, entry_id: &str) -> (Value, String) {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let verdict = verdicts
        .iter()
        .find(|held| held.entry.id == PyStr::from_str(entry_id))
        .expect("verdict stands");
    (kill_call_request(verdict), text.to_string())
}

struct StubSource {
    answer: Option<Value>,
    requests: RefCell<Vec<Value>>,
}

impl StubSource {
    fn new(answer: Option<Value>) -> StubSource {
        StubSource {
            answer,
            requests: RefCell::new(Vec::new()),
        }
    }
}

impl JudgmentSource for StubSource {
    fn judge(&self, request: &Value) -> Option<Value> {
        self.requests.borrow_mut().push(request.clone());
        self.answer.clone()
    }
}

fn plan_with(
    entry_id: &str,
    source: &dyn JudgmentSource,
    text: &str,
) -> zombie_hunt::errors::Result<String> {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    build_plan(
        &snap,
        &classify(&snap).expect("classification stands"),
        &PyStr::from_str(entry_id),
        Some(source),
    )
}

// ZH-14. The request is the proof pack and nothing else.

#[test]
fn the_kill_call_request_carries_the_proof_pack() {
    let (request, _) = verdict_and_snap(SNAPSHOT_TEXT, "stale-1");
    assert_eq!(canonical(&request), STALE_1_REQUEST_CANONICAL);
}

#[test]
fn the_request_is_built_from_the_verdict_alone() {
    let (first, _) = verdict_and_snap(SNAPSHOT_TEXT, "stale-1");
    let (second, _) = verdict_and_snap(SNAPSHOT_TEXT, "stale-1");
    assert_eq!(canonical(&first), canonical(&second));
    assert_eq!(canonical(&first), canonical(&v(REQUEST_JSON)));
}

// ZH-15. The fixture shape, refused at load defect by defect.

#[test]
fn a_well_formed_fixture_validates() {
    let data = v(&pair(REQUEST_JSON, QUARANTINE_JSON));
    validate_fixture(&data, "fixture").expect("well formed");
    assert_eq!(
        canonical(data.get("response").expect("response")),
        canonical(&v(QUARANTINE_JSON))
    );
}

#[test]
fn probabilities_within_the_sum_tolerance_validate() {
    let response = "{\"kind\": \"choice\", \"choice\": \"quarantine\", \"confidence\": 0.9, \"probabilities\": {\"quarantine\": 0.334, \"watch\": 0.333, \"spare\": 0.333}}";
    validate_fixture(&v(&pair(REQUEST_JSON, response)), "fixture").expect("within tolerance");
}

#[test]
fn a_fixture_outside_the_shape_is_refused_at_load() {
    let cases: Vec<(String, &str)> = vec![
        (
            format!(
                "{{\"synthetic\": true, \"judgment_point\": \"kill-call\", \"request\": {REQUEST_JSON}, \"response\": {QUARANTINE_JSON}, \"comment\": \"recorded by hand\"}}"
            ),
            "'comment'",
        ),
        (
            format!(
                "{{\"synthetic\": true, \"judgment_point\": \"kill-call\", \"request\": {REQUEST_JSON}}}"
            ),
            "carries no response",
        ),
        (
            pair(REQUEST_JSON, QUARANTINE_JSON).replace("\"synthetic\": true", "\"synthetic\": false"),
            "not marked synthetic",
        ),
        (
            pair(REQUEST_JSON, QUARANTINE_JSON)
                .replace("\"judgment_point\": \"kill-call\"", "\"judgment_point\": \"classification\""),
            "not the kill-call point",
        ),
        (
            pair(
                &REQUEST_JSON.replace("\"entry\": \"stale-1\"", "\"entry\": \"stale-1\", \"extra\": \"x\""),
                QUARANTINE_JSON,
            ),
            "does not carry exactly the fields",
        ),
        (
            pair(
                &REQUEST_JSON.replace(", \"evidence\": \"signal age 3600 seconds reaches the proof threshold of 360 seconds\"", ""),
                QUARANTINE_JSON,
            ),
            "outcome and evidence",
        ),
        (
            pair(
                &REQUEST_JSON.replace(
                    "\"owner\": {\"outcome\": \"satisfied\"",
                    "\"owner\": {\"outcome\": \"proven\"",
                ),
                QUARANTINE_JSON,
            ),
            "not satisfied, contradicted, or unknown",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"kind\": \"choice\"", "\"kind\": \"choice\", \"notes\": \"sure\""),
            ),
            "'notes'",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"kind\": \"choice\"", "\"kind\": \"abstention\""),
            ),
            "'choice'",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"choice\": \"quarantine\"", "\"choice\": \"kill\""),
            ),
            "not one of the labels",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace(", \"spare\": 0.03", ""),
            ),
            "do not cover exactly the labels",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"spare\": 0.03", "\"spare\": 0.03, \"extra\": 0.0"),
            ),
            "do not cover exactly the labels",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"spare\": 0.03", "\"spare\": 0.02"),
            ),
            "do not add up to 1",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": \"0.9\""),
            ),
            "not a number between 0 and 1",
        ),
        (
            pair(
                REQUEST_JSON,
                &QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": 1.5"),
            ),
            "not a number between 0 and 1",
        ),
    ];
    for (text, fragment) in cases {
        match validate_fixture(&v(&text), "fixture") {
            Err(Error::Judgment(message)) => {
                assert!(
                    message.contains(fragment),
                    "message {message:?} lacks {fragment:?}"
                );
            }
            other => panic!("expected a judgment refusal for {fragment:?}, got {other:?}"),
        }
    }
}

#[test]
fn a_non_string_kind_is_refused_at_load_and_is_no_judgment() {
    let bad = v(&QUARANTINE_JSON.replace("\"kind\": \"choice\"", "\"kind\": []"));
    assert!(matches!(
        validate_fixture(&v(&pair(REQUEST_JSON, &canonical(&bad))), "fixture"),
        Err(Error::Judgment(_))
    ));
    assert_eq!(check_answer(Some(bad)), None);
}

#[test]
fn an_unconvertible_number_is_refused_and_is_no_judgment() {
    let huge = format!("1{}", "0".repeat(399));
    let bad =
        v(&QUARANTINE_JSON.replace("\"confidence\": 0.9", &format!("\"confidence\": {huge}")));
    assert!(matches!(
        validate_fixture(&v(&pair(REQUEST_JSON, &canonical(&bad))), "fixture"),
        Err(Error::Judgment(_))
    ));
    assert_eq!(check_answer(Some(bad)), None);
    let nan = v(&QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": NaN"));
    assert_eq!(check_answer(Some(nan)), None);
}

// ZH-16. The player loads, validates, matches exactly, and abstains.

fn write_fixtures(dir: &Path, pairs: &[String]) -> String {
    for (index, data) in pairs.iter().enumerate() {
        common::write(dir, &format!("fixture-{index}.json"), data);
    }
    dir.to_str().expect("utf8").to_string()
}

#[test]
fn the_player_answers_a_recorded_request_and_abstains_elsewhere() {
    let dir = common::test_dir("judgment-player");
    let player = FixturePlayer::load(
        write_fixtures(&dir, &[pair(REQUEST_JSON, QUARANTINE_JSON)]).as_bytes(),
    )
    .expect("player loads");
    let request = v(REQUEST_JSON);
    assert_eq!(
        canonical(&player.judge(&request).expect("recorded answer")),
        canonical(&v(QUARANTINE_JSON))
    );
    let other = v(&REQUEST_JSON.replace("\"entry\": \"stale-1\"", "\"entry\": \"stale-2\""));
    assert_eq!(player.judge(&other), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_fixtures_for_one_request_refuse_the_load() {
    let dir = common::test_dir("judgment-duplicate");
    let text = pair(REQUEST_JSON, QUARANTINE_JSON);
    match FixturePlayer::load(write_fixtures(&dir, &[text.clone(), text]).as_bytes()) {
        Err(Error::Judgment(message)) => {
            assert!(message.contains("records the same request"), "{message:?}");
        }
        other => panic!("expected a load refusal, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_fixtures_directory_is_refused_at_load() {
    let dir = common::test_dir("judgment-missing");
    let absent = dir.join("absent");
    assert!(matches!(
        FixturePlayer::load(absent.to_str().expect("utf8").as_bytes()),
        Err(Error::Judgment(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_fixture_that_is_not_json_is_refused_at_load() {
    let dir = common::test_dir("judgment-not-json");
    common::write(&dir, "fixture-0.json", "not json");
    assert!(matches!(
        FixturePlayer::load(dir.to_str().expect("utf8").as_bytes()),
        Err(Error::Judgment(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_committed_fixtures_load_and_cover_the_five_answers() {
    let fixtures = root().join("fixtures");
    let player =
        FixturePlayer::load(fixtures.to_str().expect("utf8").as_bytes()).expect("player loads");
    let judge = |text: &str, entry_id: &str| {
        let (request, _) = verdict_and_snap(text, entry_id);
        player.judge(&request)
    };
    let stale_1 = judge(SNAPSHOT_TEXT, "stale-1").expect("stale-1 answer");
    assert_eq!(
        stale_1
            .get("choice")
            .and_then(|c| c.as_str())
            .map(|s| s.to_display()),
        Some("quarantine".to_string())
    );
    let stale_2 = judge(SNAPSHOT_TEXT, "stale-2").expect("stale-2 answer");
    assert_eq!(
        stale_2
            .get("choice")
            .and_then(|c| c.as_str())
            .map(|s| s.to_display()),
        Some("watch".to_string())
    );
    let spare = judge(SNAPSHOT_B, "orphan-9").expect("orphan answer");
    assert_eq!(
        spare
            .get("choice")
            .and_then(|c| c.as_str())
            .map(|s| s.to_display()),
        Some("spare".to_string())
    );
    assert_eq!(spare.get("confidence").and_then(|c| c.as_f64()), Some(0.75));
    let abstention = judge(SNAPSHOT_B, "derelict-3").expect("derelict-3 answer");
    assert_eq!(canonical(&abstention), "{\"kind\": \"abstention\"}");
    let below = judge(SNAPSHOT_B, "derelict-4").expect("derelict-4 answer");
    assert_eq!(
        below
            .get("choice")
            .and_then(|c| c.as_str())
            .map(|s| s.to_display()),
        Some("quarantine".to_string())
    );
    assert_eq!(below.get("confidence").and_then(|c| c.as_f64()), Some(0.45));
    assert!(!trusted(Some(&below)));
}

// ZH-17. Trust stands at the threshold and nowhere else.

#[test]
fn trust_reaches_at_exactly_point_six() {
    let at = v(&QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": 0.6"));
    let under = v(&QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": 0.599999"));
    assert!(trusted(Some(&at)));
    assert!(!trusted(Some(&under)));
    assert_eq!(check_answer(Some(under.clone())), Some(under));
    assert!(!trusted(Some(&v("{\"kind\": \"abstention\"}"))));
    assert!(!trusted(None));
}

// ZH-17. Trust passes the shape check first, whoever
// handed the answer back, so an answer outside the
// recorded shape is never trusted however high its
// confidence stands.

#[test]
fn trust_refuses_an_answer_outside_the_recorded_shape() {
    // The witness answer, a choice kind carrying a
    // confidence past the unit interval and neither a
    // choice nor probabilities standing.
    assert!(!trusted(Some(&v(
        "{\"kind\": \"choice\", \"confidence\": 1.5}"
    ))));
    // A confidence past the unit interval on its own.
    assert!(!trusted(Some(&v(
        "{\"kind\": \"choice\", \"choice\": \"watch\", \"confidence\": 1.5, \
\"probabilities\": {\"quarantine\": 0.1, \"watch\": 0.8, \"spare\": 0.1}}"
    ))));
    // The probabilities missing on their own.
    assert!(!trusted(Some(&v(
        "{\"kind\": \"choice\", \"choice\": \"watch\", \"confidence\": 0.9}"
    ))));
    // And the well formed answer at the same confidence
    // still stands, so the refusals are the shape's.
    assert!(trusted(Some(&v(
        "{\"kind\": \"choice\", \"choice\": \"watch\", \"confidence\": 0.9, \
\"probabilities\": {\"quarantine\": 0.1, \"watch\": 0.8, \"spare\": 0.1}}"
    ))));
}

// ZH-18. The plan rule, asked late, recorded exactly, or untouched.

#[test]
fn a_refused_plan_asks_nothing() {
    for entry_id in ["live-1", "busy-1", "done-run", "no-such-entry"] {
        let spy = StubSource::new(Some(v(QUARANTINE_JSON)));
        let snap = parse_snapshot(
            &PyStr::from_str(SNAPSHOT_TEXT),
            &PyStr::from_str("snap.txt"),
        )
        .expect("snapshot parses");
        let result = build_plan(
            &snap,
            &classify(&snap).expect("classification stands"),
            &PyStr::from_str(entry_id),
            Some(&spy),
        );
        assert!(
            matches!(result, Err(Error::Plan(_))),
            "{entry_id} must refuse"
        );
        assert!(
            spy.requests.borrow().is_empty(),
            "a refused plan asks nothing"
        );
    }
}

#[test]
fn a_trusted_quarantine_answer_advances_the_plan() {
    let spy = StubSource::new(Some(v(QUARANTINE_JSON)));
    assert_eq!(
        plan_with("stale-1", &spy, SNAPSHOT_TEXT).expect("plan"),
        ADVANCED_PLAN
    );
    let requests = spy.requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(canonical(&requests[0]), STALE_1_REQUEST_CANONICAL);
}

#[test]
fn a_trusted_watch_answer_is_recorded_unadvanced() {
    let watch = v(
        "{\"kind\": \"choice\", \"choice\": \"watch\", \"confidence\": 0.8, \"probabilities\": {\"quarantine\": 0.1, \"watch\": 0.8, \"spare\": 0.1}}",
    );
    let text = plan_with("stale-2", &StubSource::new(Some(watch)), SNAPSHOT_TEXT).expect("plan");
    assert_eq!(text, WATCH_PLAN);
    assert!(!text.contains("advances this plan"));
}

#[test]
fn a_trusted_spare_answer_is_recorded_unadvanced() {
    let spare = v(
        "{\"kind\": \"choice\", \"choice\": \"spare\", \"confidence\": 0.75, \"probabilities\": {\"quarantine\": 0.15, \"watch\": 0.1, \"spare\": 0.75}}",
    );
    let text = plan_with("stale-1", &StubSource::new(Some(spare)), SNAPSHOT_TEXT).expect("plan");
    assert!(text.contains("kill call verdict spare with confidence 0.75"));
    assert!(text.contains("verdict leaves this plan unadvanced"));
    assert!(!text.contains("advances this plan"));
    assert!(text.ends_with(
        "executes nothing, this plan is a document and the tool performs no part of it\n"
    ));
}

#[test]
fn an_abstention_leaves_the_round_1_plan_byte_identical() {
    let spy = StubSource::new(Some(v("{\"kind\": \"abstention\"}")));
    assert_eq!(
        plan_with("stale-1", &spy, SNAPSHOT_TEXT).expect("plan"),
        EXPECTED_PLAN
    );
    assert_eq!(spy.requests.borrow().len(), 1);
}

#[test]
fn a_below_threshold_answer_leaves_the_round_1_plan() {
    let below = v(
        "{\"kind\": \"choice\", \"choice\": \"quarantine\", \"confidence\": 0.45, \"probabilities\": {\"quarantine\": 0.45, \"watch\": 0.35, \"spare\": 0.2}}",
    );
    assert_eq!(
        plan_with("stale-1", &StubSource::new(Some(below)), SNAPSHOT_TEXT).expect("plan"),
        EXPECTED_PLAN
    );
}

#[test]
fn an_answer_at_exactly_point_six_advances_the_plan() {
    let at = v(&QUARANTINE_JSON.replace("\"confidence\": 0.9", "\"confidence\": 0.6"));
    let text = plan_with("stale-1", &StubSource::new(Some(at)), SNAPSHOT_TEXT).expect("plan");
    assert!(text.contains("kill call verdict quarantine with confidence 0.6"));
    assert!(text.contains("verdict advances this plan"));
}

#[test]
fn a_hostile_answer_counts_as_no_judgment() {
    let huge = format!("1{}", "0".repeat(399));
    let answers: Vec<Option<Value>> = vec![
        Some(v(
            &QUARANTINE_JSON.replace("\"kind\": \"choice\"", "\"kind\": []")
        )),
        Some(v(&QUARANTINE_JSON.replace(
            "\"confidence\": 0.9",
            &format!("\"confidence\": {huge}"),
        ))),
        Some(v(&QUARANTINE_JSON.replace(
            "\"kind\": \"choice\"",
            "\"kind\": \"choice\", \"notes\": \"sure\"",
        ))),
        Some(v(&QUARANTINE_JSON.replace(
            "\"choice\": \"quarantine\"",
            "\"choice\": \"kill\"",
        ))),
        Some(v(
            "{\"kind\": \"score\", \"score\": 0.9, \"confidence\": 0.9}",
        )),
        Some(v("\"quarantine\"")),
        None,
    ];
    for answer in answers {
        assert_eq!(
            plan_with("stale-1", &StubSource::new(answer), SNAPSHOT_TEXT).expect("plan"),
            EXPECTED_PLAN
        );
    }
}

#[test]
fn the_verdict_never_touches_the_plan_above_it() {
    let text = plan_with(
        "stale-1",
        &StubSource::new(Some(v(QUARANTINE_JSON))),
        SNAPSHOT_TEXT,
    )
    .expect("plan");
    let head = EXPECTED_PLAN
        .split("executes nothing")
        .next()
        .expect("head");
    assert!(text.starts_with(head));
    assert_eq!(
        text.matches("step ").count(),
        EXPECTED_PLAN.matches("step ").count()
    );
}

#[test]
fn the_committed_player_drives_the_plan_end_to_end() {
    let fixtures = root().join("fixtures");
    let player =
        FixturePlayer::load(fixtures.to_str().expect("utf8").as_bytes()).expect("player loads");
    assert_eq!(
        plan_with("stale-1", &player, SNAPSHOT_TEXT).expect("plan"),
        ADVANCED_PLAN
    );
    assert_eq!(
        plan_with("stale-2", &player, SNAPSHOT_TEXT).expect("plan"),
        WATCH_PLAN
    );
    for entry_id in ["orphan-9", "derelict-3", "derelict-4"] {
        let with_verdict = plan_with(entry_id, &player, SNAPSHOT_B).expect("plan");
        let snap = parse_snapshot(&PyStr::from_str(SNAPSHOT_B), &PyStr::from_str("snap.txt"))
            .expect("snapshot parses");
        let plain = build_plan(
            &snap,
            &classify(&snap).expect("classification stands"),
            &PyStr::from_str(entry_id),
            None,
        )
        .expect("plain plan");
        if entry_id == "orphan-9" {
            assert!(with_verdict.contains("kill call verdict spare"));
            assert_ne!(with_verdict, plain);
        } else {
            assert_eq!(with_verdict, plain);
        }
    }
}

// ZH-19 and the CLI. The scan never consults, load failures refuse.

#[test]
fn the_scan_is_byte_identical_with_the_seam_built() {
    let dir = common::test_dir("judgment-cli-scan");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let outcome = common::run_cli(&["scan", snap.to_str().expect("utf8")], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), common::EXPECTED_SCAN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_plan_without_fixtures_matches_round_1() {
    let dir = common::test_dir("judgment-cli-plain");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let outcome = common::run_cli(&["plan", snap.to_str().expect("utf8"), "stale-1"], &dir);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout_text(), EXPECTED_PLAN);
    assert_eq!(outcome.stderr_text(), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_plan_with_fixtures_records_the_verdicts() {
    let dir = common::test_dir("judgment-cli-fixtures");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let fixtures = root().join("fixtures");
    let fixtures = fixtures.to_str().expect("utf8");
    let advanced = common::run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "stale-1",
            "--fixtures",
            fixtures,
        ],
        &dir,
    );
    assert_eq!(advanced.code, 0);
    assert_eq!(advanced.stdout_text(), ADVANCED_PLAN);
    let watched = common::run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "stale-2",
            "--fixtures",
            fixtures,
        ],
        &dir,
    );
    assert_eq!(watched.code, 0);
    assert_eq!(watched.stdout_text(), WATCH_PLAN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cli_plan_refusal_stands_with_fixtures_present() {
    let dir = common::test_dir("judgment-cli-refusal");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let fixtures = root().join("fixtures");
    let outcome = common::run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "busy-1",
            "--fixtures",
            fixtures.to_str().expect("utf8"),
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(outcome.stderr_text().contains("classified suspect"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_fixtures_directory_exits_two_with_one_line() {
    let dir = common::test_dir("judgment-cli-missing");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let absent = dir.join("absent");
    let outcome = common::run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "stale-1",
            "--fixtures",
            absent.to_str().expect("utf8"),
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(!outcome.stderr_text().contains("Traceback"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_malformed_fixture_exits_two_with_one_line() {
    let dir = common::test_dir("judgment-cli-malformed");
    let snap = common::write(&dir, "snap.txt", SNAPSHOT_TEXT);
    let bad = pair(
        REQUEST_JSON,
        &QUARANTINE_JSON.replace("\"choice\": \"quarantine\"", "\"choice\": \"kill\""),
    );
    common::write(&dir, "bad.json", &bad);
    let outcome = common::run_cli(
        &[
            "plan",
            snap.to_str().expect("utf8"),
            "stale-1",
            "--fixtures",
            dir.to_str().expect("utf8"),
        ],
        &dir,
    );
    assert_eq!(outcome.code, 2);
    assert_eq!(outcome.stdout_text(), "");
    assert_eq!(outcome.stderr_text().lines().count(), 1);
    assert!(!outcome.stderr_text().contains("Traceback"));
    let _ = std::fs::remove_dir_all(&dir);
}
