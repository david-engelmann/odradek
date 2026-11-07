//! ZH-20. The holds list as a set, and the permutation invariance.

mod common;

use std::path::PathBuf;

use odradek::judgment::{FixturePlayer, JudgmentSource, kill_call_request};
use odradek::plan::build_plan;
use odradek::pyjson::dumps_canonical;
use odradek::pytext::PyStr;
use odradek::scan::{CONTRADICTED, PROVEN, SUSPECT, classify, format_scan};
use odradek::snapshot::parse_snapshot;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn example() -> String {
    std::fs::read_to_string(root().join("examples").join("snapshot.txt")).expect("read example")
}

fn player() -> FixturePlayer {
    FixturePlayer::load(root().join("fixtures").to_str().expect("utf8").as_bytes())
        .expect("player loads")
}

fn scan_document(text: &str) -> String {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    format_scan(&snap, &classify(&snap).expect("classification stands"))
}

fn plan_document(text: &str, entry_id: &str, source: Option<&dyn JudgmentSource>) -> String {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    build_plan(
        &snap,
        &classify(&snap).expect("classification stands"),
        &PyStr::from_str(entry_id),
        source,
    )
    .expect("plan")
}

fn canonical_request(text: &str, entry_id: &str) -> String {
    let snap = parse_snapshot(&PyStr::from_str(text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let verdict = verdicts
        .iter()
        .find(|held| held.entry.id == PyStr::from_str(entry_id))
        .expect("verdict stands");
    let mut out = String::new();
    dumps_canonical(&kill_call_request(verdict), &mut out);
    out
}

const STALE_HOLDS: &str = "holds cache\nowner done-run";

#[test]
fn a_repeated_plain_token_scans_byte_identically_to_one_token() {
    let example = example();
    assert_eq!(example.matches(STALE_HOLDS).count(), 1);
    let repeated = example.replace(STALE_HOLDS, "holds cache,cache\nowner done-run");
    let scan = scan_document(&repeated);
    assert_eq!(scan, scan_document(&example));
    assert!(scan.contains("holds cache with no in-flight marker"));
    assert!(!scan.contains("cache, cache"));
}

#[test]
fn a_repeated_plain_token_plans_byte_identically_to_one_token() {
    let example = example();
    let repeated = example.replace(STALE_HOLDS, "holds cache,cache\nowner done-run");
    let plan = plan_document(&repeated, "stale-1", None);
    assert_eq!(plan, plan_document(&example, "stale-1", None));
    assert!(plan.contains("fence the recorded holds, cache, so"));
    assert!(!plan.contains("cache, cache"));
}

#[test]
fn a_repeated_token_builds_the_single_token_kill_call_request() {
    let example = example();
    let repeated = example.replace(STALE_HOLDS, "holds cache,cache\nowner done-run");
    assert_eq!(
        canonical_request(&repeated, "stale-1"),
        canonical_request(&example, "stale-1")
    );
}

#[test]
fn a_repeated_token_plan_matches_the_recorded_fixture_verdict() {
    let example = example();
    let player = player();
    let repeated = example.replace(STALE_HOLDS, "holds cache,cache\nowner done-run");
    let plan = plan_document(&repeated, "stale-1", Some(&player));
    assert_eq!(plan, plan_document(&example, "stale-1", Some(&player)));
    assert!(plan.contains("kill call verdict quarantine with confidence 0.9"));
    assert!(plan.contains("verdict advances this plan"));
}

#[test]
fn a_repeated_in_flight_marker_scans_byte_identically_to_one() {
    let example = example();
    assert_eq!(example.matches("holds in-flight,cache").count(), 1);
    let single = example.replace("holds in-flight,cache", "holds in-flight");
    let repeated = example.replace("holds in-flight,cache", "holds in-flight,in-flight");
    let scan = scan_document(&repeated);
    assert_eq!(scan, scan_document(&single));
    assert!(scan.contains("holds in-flight including the in-flight marker"));
    assert!(!scan.contains("in-flight, in-flight"));
}

#[test]
fn an_entry_holding_its_own_identifier_is_unremarkable() {
    let example = example();
    let text = example.replace(STALE_HOLDS, "holds stale-1\nowner done-run");
    let snap = parse_snapshot(&PyStr::from_str(&text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let verdict = verdicts
        .iter()
        .find(|held| held.entry.id == PyStr::from_str("stale-1"))
        .expect("verdict stands");
    assert_eq!(verdict.classification, PROVEN);
    assert_eq!(
        verdict.in_flight.fact,
        "holds stale-1 with no in-flight marker"
    );
    assert!(scan_document(&text).contains("zombie stale-1"));
}

#[test]
fn an_entry_naming_itself_as_owner_is_at_most_a_suspect() {
    let example = example();
    let text = example.replace(STALE_HOLDS, "holds cache\nowner stale-1");
    let snap = parse_snapshot(&PyStr::from_str(&text), &PyStr::from_str("snap.txt"))
        .expect("snapshot parses");
    let verdicts = classify(&snap).expect("classification stands");
    let verdict = verdicts
        .iter()
        .find(|held| held.entry.id == PyStr::from_str("stale-1"))
        .expect("verdict stands");
    assert_eq!(verdict.classification, SUSPECT);
    assert_eq!(verdict.owner.status, CONTRADICTED);
    assert_eq!(verdict.owner.fact, "owner stale-1 is recorded running");
    let scan = scan_document(&text);
    assert!(scan.contains("suspect stale-1"));
    assert!(scan.contains("evidence against owner stale-1 is recorded running"));
}

fn split_blocks(text: &str) -> (String, Vec<Vec<String>>) {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let mut blocks: Vec<Vec<String>> = Vec::new();
    for line in &lines[1..] {
        if line.starts_with("entry ") {
            blocks.push(vec![line.clone()]);
        } else {
            blocks
                .last_mut()
                .expect("a block stands")
                .push(line.clone());
        }
    }
    (lines[0].clone(), blocks)
}

fn rotated_block(block: &[String], amount: usize) -> String {
    let head = &block[0];
    let mut fields: Vec<String> = block[1..].to_vec();
    if !fields.is_empty() {
        let amount = amount % fields.len();
        fields.rotate_left(amount);
    }
    let mut lines = vec![head.clone()];
    for line in fields {
        if line.starts_with("holds ") && line.contains(',') {
            let (name, value) = line.split_once(' ').expect("holds line");
            let tokens: Vec<&str> = value.split(',').collect();
            let reversed: Vec<&str> = tokens.iter().rev().copied().collect();
            lines.push(format!("{name} {}", reversed.join(",")));
        } else {
            lines.push(line);
        }
    }
    lines.join("\n")
}

/// Advance a permutation to the next in lexicographic order,
/// returning false at the last.
fn next_permutation(values: &mut [usize]) -> bool {
    if values.len() < 2 {
        return false;
    }
    let mut i = values.len() - 2;
    loop {
        if values[i] < values[i + 1] {
            let mut j = values.len() - 1;
            while values[j] <= values[i] {
                j -= 1;
            }
            values.swap(i, j);
            values[i + 1..].reverse();
            return true;
        }
        if i == 0 {
            return false;
        }
        i -= 1;
    }
}

#[test]
fn all_40320_block_permutations_of_the_example_scan_identically() {
    let example = example();
    let (header, blocks) = split_blocks(&example);
    assert_eq!(blocks.len(), 8);
    let baseline = scan_document(&example);
    let mut order: Vec<usize> = (0..8).collect();
    let mut count = 0usize;
    loop {
        let mut parts = vec![header.clone()];
        for (position, block_index) in order.iter().enumerate() {
            parts.push(rotated_block(&blocks[*block_index], count + position));
        }
        let text = parts.join("\n") + "\n";
        assert_eq!(scan_document(&text), baseline, "permutation {count}");
        count += 1;
        if !next_permutation(&mut order) {
            break;
        }
    }
    assert_eq!(count, 40320);
}

#[test]
fn all_24_block_permutations_of_a_subset_plan_identically() {
    let example = example();
    let (header, blocks) = split_blocks(&example);
    let chosen: Vec<&Vec<String>> = ["live-1", "boss-run", "done-run", "stale-1"]
        .iter()
        .map(|name| {
            blocks
                .iter()
                .find(|block| block[0] == format!("entry {name}"))
                .expect("block stands")
        })
        .collect();
    let recorded = format!(
        "{}\n{}\n",
        header,
        chosen
            .iter()
            .map(|block| block.join("\n"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let player = player();
    let baseline_scan = scan_document(&recorded);
    let baseline_plan = plan_document(&recorded, "stale-1", None);
    let baseline_verdict_plan = plan_document(&recorded, "stale-1", Some(&player));
    assert!(baseline_verdict_plan.contains("kill call verdict quarantine"));
    let mut order: Vec<usize> = (0..4).collect();
    let mut count = 0usize;
    loop {
        let text = format!(
            "{}\n{}\n",
            header,
            order
                .iter()
                .map(|index| chosen[*index].join("\n"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(scan_document(&text), baseline_scan);
        assert_eq!(plan_document(&text, "stale-1", None), baseline_plan);
        assert_eq!(
            plan_document(&text, "stale-1", Some(&player)),
            baseline_verdict_plan
        );
        count += 1;
        if !next_permutation(&mut order) {
            break;
        }
    }
    assert_eq!(count, 24);
}

#[test]
fn a_plan_prints_a_zombies_holds_sorted_however_they_were_recorded() {
    let example = example();
    let recorded = example.replace(STALE_HOLDS, "holds vault,cache\nowner done-run");
    let shuffled = example.replace(STALE_HOLDS, "holds cache,vault\nowner done-run");
    let plan = plan_document(&recorded, "stale-1", None);
    assert_eq!(plan, plan_document(&shuffled, "stale-1", None));
    let holds_lines: Vec<&str> = plan.lines().filter(|line| line.contains("vault")).collect();
    assert!(!holds_lines.is_empty());
    assert!(
        holds_lines.iter().all(|line| line.contains("cache, vault")),
        "{holds_lines:?}"
    );
}
