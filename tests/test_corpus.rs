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
    assert_eq!((oracle, clap), (105, 27), "corpus source split");
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
