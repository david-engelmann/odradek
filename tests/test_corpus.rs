//! The CLI corpus: recorded invocations run against the real
//! binary. The oracle half was produced by the Python tool the
//! crate replaces; the clap half pins the help and usage
//! presentation the contract amendment hands to clap, together
//! with the uncaught family lines the port owns. The
//! per-source split is pinned so a case cannot silently change
//! which side of that line it sits on.

mod common;

use common::{load_corpus, run_corpus_case};

#[test]
fn the_corpus_runs_and_its_split_is_pinned() {
    let cases = load_corpus();
    let oracle = cases.iter().filter(|case| case.source == "oracle").count();
    let clap = cases.iter().filter(|case| case.source == "clap").count();
    // Round 6 removes one clap case, usage-plan-missing-entry,
    // because its invocation is the estate form now and no
    // longer a usage refusal. The estate form is pinned in
    // tests/test_estate.rs instead.
    // Round 10 adds three clap cases, help-scan-short,
    // help-plan-short, and help-observe-short, firing the
    // short help spelling behind each subcommand (ZH-55).
    // Round 13 adds nine oracle cases firing the observe
    // branches the ZH-66 census marked unfired (ZH-67).
    // Round 14 adds eighteen oracle cases firing the snapshot
    // reader's refusal families behind scan and plan (ZH-71).
    // Round 31 adds six oracle cases firing the observe
    // command's refusal and read order pairings (ZH-139).
    // Round 32 adds six oracle cases firing the plan
    // command's stage order pairings (ZH-143).
    // Round 33 adds six oracle cases firing the observe
    // command's read stage order pairings (ZH-148).
    // Round 34 adds six oracle cases firing the plan
    // command's dispatch stage pairings (ZH-153).
    // Round 35 adds seven oracle cases firing the plan
    // command's build pairings that remained (ZH-158).
    // Round 36 adds six oracle cases firing the fixture
    // player's answer edges and directory mechanics behind
    // the plan command (ZH-163).
    // Round 38 adds six oracle cases firing the folded
    // snapshot path forms behind plan and observe (ZH-172).
    // Round 39 adds six oracle cases firing the fixture
    // reader's repeated field readings behind the plan
    // command (ZH-177).
    // Round 40 adds six oracle cases firing the fixture
    // reader's written number forms behind the plan
    // command (ZH-182).
    // Round 41 adds six oracle cases firing the fixture
    // reader's string escape forms behind the plan
    // command (ZH-187).
    // Round 42 adds six oracle cases firing the snapshot
    // reader's whitespace forms behind the scan command
    // (ZH-192).
    // Round 43 adds six oracle cases firing the fixture
    // reader's whitespace forms behind the plan command
    // (ZH-197).
    // Round 44 adds six oracle cases firing the snapshot
    // reader's whitespace forms behind the plan command
    // (ZH-202).
    // Round 45 adds six oracle cases firing the snapshot
    // reader's whitespace forms behind the observe command
    // (ZH-207).
    assert_eq!((oracle, clap), (356, 27), "corpus source split");
    let mut failures = Vec::new();
    for case in &cases {
        if let Err(detail) = run_corpus_case(case) {
            failures.push(format!("{}: {detail}", case.name));
        }
    }
    assert!(
        failures.is_empty(),
        "corpus failures:\n{}",
        failures.join("\n")
    );
}
