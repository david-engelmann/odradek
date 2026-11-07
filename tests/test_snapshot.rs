//! ZH-1. The recorded snapshot format and its refusals.

use odradek::errors::Error;
use odradek::pytext::PyStr;
use odradek::snapshot::parse_snapshot;

const VALID: &str = "snapshot 100000\nentry proc-1\nkind process\nstarter scheduler-1\nstate running\nsignal-interval 60\nlast-signal 99980\nholds cache,reports\nowner run-1\nentry run-1\nkind run\nstarter supervisor-1\nstate running\nsignal-interval 30\nlast-signal 99995\nholds none\nowner none\n";

fn parse(text: &str) -> odradek::errors::Result<odradek::Snapshot> {
    parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
}

#[test]
fn a_valid_snapshot_parses_with_its_facts_intact() {
    let snap = parse(VALID).expect("valid snapshot");
    assert_eq!(snap.time.as_str(), "100000");
    let ids: Vec<String> = snap.entries.iter().map(|e| e.id.to_display()).collect();
    assert_eq!(ids, ["proc-1", "run-1"]);
    let proc = snap.entry(&PyStr::from_str("proc-1")).expect("proc-1");
    assert_eq!(proc.kind, "process");
    assert_eq!(proc.starter, "scheduler-1");
    assert_eq!(proc.signal_interval.as_str(), "60");
    assert_eq!(proc.last_signal.as_ref().map(|n| n.as_str()), Some("99980"));
    assert_eq!(
        proc.holds,
        [PyStr::from_str("cache"), PyStr::from_str("reports")]
    );
    assert_eq!(proc.owner, Some(PyStr::from_str("run-1")));
    let run = snap.entry(&PyStr::from_str("run-1")).expect("run-1");
    assert!(run.holds.is_empty());
    assert_eq!(run.owner, None);
}

#[test]
fn comments_blank_lines_and_field_order_change_nothing() {
    let shuffled = "# a recorded snapshot, synthetic\n\nsnapshot 100000\nentry proc-1\nowner run-1\nholds cache,reports\nlast-signal 99980\nsignal-interval 60\nstate running\nstarter scheduler-1\nkind process\nentry run-1\nkind run\nstarter supervisor-1\nstate running\nsignal-interval 30\nlast-signal 99995\nholds none\nowner none\n";
    assert_eq!(
        parse(shuffled).expect("shuffled"),
        parse(VALID).expect("valid")
    );
}

#[test]
fn malformed_snapshots_are_refused_with_the_defect_named() {
    let refusals: [(&str, &str); 17] = [
        ("", "no snapshot header"),
        ("entry proc-1\nkind process\n", "not a snapshot header"),
        ("snapshot 100000\nsnapshot 100001\n", "more than once"),
        ("snapshot soon\n", "not a whole number"),
        ("snapshot 100000\nkind process\n", "before any entry block"),
        (
            "snapshot 100000\nentry proc-1 extra\n",
            "exactly one identifier",
        ),
        (
            "snapshot 100000\nentry proc-1\nkind process\nkind run\n",
            "carries the kind line twice",
        ),
        (
            "snapshot 100000\nentry proc-1\nweight heavy\n",
            "matches no field form",
        ),
        (
            "snapshot 100000\nentry proc-1\nkind process\n",
            "carries no starter line",
        ),
        // The entered-twice and value cases derive from VALID.
        ("VALID+entry", "is entered twice"),
        ("VALID-interval-0", "not positive"),
        ("VALID-interval-soon", "not a whole number of seconds"),
        ("VALID-signal-late", "later than the snapshot time"),
        ("VALID-kind", "neither process nor run"),
        ("VALID-state", "neither running nor finished"),
        ("VALID-holds-none", "the word none inside the list"),
        ("VALID-holds-empty", "an empty token"),
    ];
    for (text, fragment) in refusals {
        let text = match text {
            "VALID+entry" => format!("{VALID}entry proc-1\nkind process\n"),
            "VALID-interval-0" => VALID.replace("signal-interval 60", "signal-interval 0"),
            "VALID-interval-soon" => VALID.replace("signal-interval 60", "signal-interval soon"),
            "VALID-signal-late" => VALID.replace("last-signal 99980", "last-signal 100001"),
            "VALID-kind" => VALID.replace("kind process", "kind daemon"),
            "VALID-state" => VALID.replacen("state running", "state paused", 1),
            "VALID-holds-none" => VALID.replace("holds cache,reports", "holds none,cache"),
            "VALID-holds-empty" => VALID.replace("holds cache,reports", "holds cache,,reports"),
            other => other.to_string(),
        };
        match parse(&text) {
            Err(Error::Snapshot(message)) => {
                assert!(
                    message.contains(fragment),
                    "message {message:?} lacks {fragment:?}"
                );
                assert!(
                    message.contains("snap.txt"),
                    "message {message:?} lacks the source"
                );
            }
            other => panic!("expected a snapshot refusal for {fragment:?}, got {other:?}"),
        }
    }
}

#[test]
fn whole_numbers_convert_digit_by_digit_across_scripts() {
    // ZH-31. An Arabic-Indic one, a Devanagari two, and an
    // ASCII three read as 123, each digit taking its value
    // inside its own block of ten.
    let text = "snapshot \u{661}\u{968}\u{33}\n";
    let snap = parse(text).expect("mixed script time");
    assert_eq!(snap.time.as_str(), "123");
    // A superscript two is category No, not Nd, and refuses.
    let refused = parse("snapshot 1\u{b2}\n");
    assert!(matches!(refused, Err(Error::Snapshot(_))));
}

#[test]
fn leading_zeros_canonicalize_away() {
    let snap = parse("snapshot 0100000\n").expect("leading zeros");
    assert_eq!(snap.time.as_str(), "100000");
}
