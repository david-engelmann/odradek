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

// ZH-58 and ZH-60. The scan at scale, its owner rule reading
// through the one index the classification builds.

#[test]
fn a_scan_of_three_hundred_entries_with_owners_matches_its_whole_document() {
    // Three hundred entries stand in the snapshot, two
    // hundred ninety nine running entries recorded in
    // reverse identifier order behind one finished keeper.
    // Every running entry's signal is long past its
    // threshold, and the owners cycle through the three
    // recorded shapes, a finished owner, an absent owner,
    // and no owner recorded, so the scan's owner resolution
    // is exercised at scale in every shape it can record.
    // The expected document is built from the construction
    // alone, the counts line and every evidence line, and
    // asserted byte for byte, so an owner resolved to the
    // wrong entry anywhere in the three hundred would move
    // the document.
    let mut text = String::from("snapshot 100000\n");
    text.push_str("entry keeper\nkind run\nstarter supervisor-1\nstate finished\nsignal-interval 300\nlast-signal 90000\nholds none\nowner none\n");
    for n in (0..299).rev() {
        let owner = match n % 3 {
            0 => "keeper",
            1 => "ghost-7",
            _ => "none",
        };
        text.push_str(&format!(
            "entry s-{n:03}\nkind process\nstarter cron-1\nstate running\nsignal-interval 60\nlast-signal 90000\nholds none\nowner {owner}\n"
        ));
    }
    let signal_line =
        "  evidence for signal age 10000 seconds reaches the proof threshold of 180 seconds\n";
    let holds_line = "  evidence for the entry holds nothing and carries no in-flight marker\n";
    let mut expected =
        String::from("snapshot 100000\ncounts live 0 suspect 99 proven 200 settled 1\n");
    for n in 0..299 {
        if n % 3 == 2 {
            continue;
        }
        expected.push_str(&format!("zombie s-{n:03}\n{signal_line}"));
        if n % 3 == 0 {
            expected.push_str("  evidence for owner keeper is recorded finished\n");
        } else {
            expected.push_str("  evidence for owner ghost-7 is absent from the snapshot\n");
        }
        expected.push_str(holds_line);
    }
    for n in 0..299 {
        if n % 3 != 2 {
            continue;
        }
        expected.push_str(&format!(
            "suspect s-{n:03}\n{signal_line}  missing no owner reference is recorded for this entry\n{holds_line}"
        ));
    }
    assert_eq!(scan(&text), expected);
}

#[test]
fn a_duplicated_owner_identifier_resolves_to_the_first_entry() {
    // A snapshot built through the public types can carry a
    // duplicated identifier the parser would refuse (ZH-1).
    // The index the classification reads through keeps the
    // first entry under the identifier, exactly as the linear
    // search in entry does, so stale-1's owner done-run reads
    // as the finished entry recorded first and never as the
    // running copy standing behind it.
    let mut snap = parse_snapshot(
        &PyStr::from_str(SNAPSHOT_TEXT),
        &PyStr::from_str("snap.txt"),
    )
    .expect("snapshot parses");
    let mut duplicate = snap
        .entries
        .iter()
        .find(|entry| entry.id == PyStr::from_str("done-run"))
        .expect("done-run stands")
        .clone();
    duplicate.state = PyStr::from_str("running");
    snap.entries.push(duplicate);
    let verdicts = classify(&snap).expect("classification stands");
    let stale = verdicts
        .iter()
        .find(|verdict| verdict.entry.id == PyStr::from_str("stale-1"))
        .expect("stale-1 stands classified");
    assert_eq!(stale.classification, "proven zombie");
    assert_eq!(stale.owner.status, "satisfied");
    assert_eq!(stale.owner.fact, "owner done-run is recorded finished");
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
