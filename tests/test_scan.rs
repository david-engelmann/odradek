//! ZH-5 to ZH-8. Classification, the scan document, and determinism.

mod common;

use odradek::pytext::PyStr;
use odradek::scan::{classify, format_scan};
use odradek::snapshot::parse_snapshot;

use common::{EXPECTED_SCAN, SNAPSHOT_TEXT};

fn scan(text: &str) -> String {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    format_scan(&snap, &classify(&snap).expect("classification stands"))
}

#[test]
fn the_scan_matches_the_expected_document_byte_for_byte() {
    assert_eq!(scan(SNAPSHOT_TEXT), EXPECTED_SCAN);
}

#[test]
fn no_suspect_is_ever_reported_as_a_zombie() {
    let snap = parse_snapshot(
        &PyStr::from_str(SNAPSHOT_TEXT),
        &PyStr::from_str("snap.txt"),
    )
    .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    for (id, class) in [
        ("busy-1", "suspect"),
        ("half-1", "suspect"),
        ("quiet-1", "suspect"),
    ] {
        let verdict = verdicts
            .iter()
            .find(|v| v.entry.id == PyStr::from_str(id))
            .expect("verdict stands");
        assert_eq!(verdict.classification, class);
    }
    let out = scan(SNAPSHOT_TEXT);
    let zombie_ids: Vec<&str> = out
        .lines()
        .filter_map(|line| line.strip_prefix("zombie "))
        .collect();
    assert_eq!(zombie_ids, ["stale-1", "stale-2"]);
}

#[test]
fn live_and_settled_entries_are_counted_but_never_listed() {
    let out = scan(SNAPSHOT_TEXT);
    assert!(out.contains("counts live 2 suspect 3 proven 2 settled 1"));
    let listed: Vec<&str> = out
        .lines()
        .filter_map(|line| {
            line.strip_prefix("zombie ")
                .or_else(|| line.strip_prefix("suspect "))
        })
        .collect();
    for absent in ["live-1", "boss-run", "done-run"] {
        assert!(!listed.contains(&absent), "{absent} must not be listed");
    }
}

#[test]
fn every_listed_entry_carries_exactly_three_evidence_lines() {
    let out = scan(SNAPSHOT_TEXT);
    let mut blocks = 0;
    let mut evidence = 0;
    for line in out.lines() {
        if line.starts_with("zombie ") || line.starts_with("suspect ") {
            if blocks > 0 {
                assert_eq!(evidence, 3, "each block carries three evidence lines");
            }
            blocks += 1;
            evidence = 0;
        } else if line.starts_with("  ") {
            evidence += 1;
        }
    }
    assert_eq!(blocks, 5);
    assert_eq!(evidence, 3);
}

#[test]
fn a_snapshot_with_nothing_to_list_prints_the_counts_line_alone() {
    let text = "snapshot 500\nentry only-live\nkind process\nstarter scheduler-1\nstate running\nsignal-interval 60\nlast-signal 490\nholds none\nowner none\n";
    assert_eq!(
        scan(text),
        "snapshot 500\ncounts live 1 suspect 0 proven 0 settled 0\n"
    );
}

#[test]
fn repeated_and_shuffled_snapshots_scan_byte_identically() {
    assert_eq!(scan(SNAPSHOT_TEXT), scan(SNAPSHOT_TEXT));
    // Reverse the entry blocks and reverse one holds list, as
    // the Python pin does.
    let blocks: Vec<&str> = SNAPSHOT_TEXT.split("entry ").skip(1).collect();
    let mut shuffled = String::from("snapshot 100000\nentry ");
    let parts: Vec<String> = blocks.iter().rev().map(|block| block.to_string()).collect();
    shuffled.push_str(&parts.join("entry "));
    let shuffled = shuffled.replace("holds in-flight,cache", "holds cache,in-flight");
    assert_eq!(scan(&shuffled), EXPECTED_SCAN);
}
