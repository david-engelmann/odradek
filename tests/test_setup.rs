//! The setup invariants, re-expressed in crate form from
//! tests/test_setup.py at the Round 4 closure (ZH-35). The
//! dash scan reads .rs where the Python form read .py, covers
//! extensionless files as well, and asserts its named anchor
//! files were actually scanned, so the scan cannot pass by
//! reading nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zombie_hunt::pytext::{decimal_value, is_decimal};

const CANONICAL_PREFIX_SHA256: &str =
    "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11";

const EXPECTED_OBLIGATIONS: [&str; 5] = [
    "ZH-S1. Every suspect on the hunt list carries the evidence that it serves nobody, and a suspect listed without evidence is a defect the tests must catch.",
    "ZH-S2. No kill fires before its proof is recorded, and an action taken on a suspect whose proof is missing or failed is a defect the tests must catch.",
    "ZH-S3. The first action on any proven zombie is a reversible quarantine, and an irreversible action as the first step on any suspect is forbidden by the contract.",
    "ZH-S4. Every hunt works from recorded or synthetic cluster state during the first slices, and no code path in those slices connects to a live cluster.",
    "ZH-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).expect("read file")
}

/// One stub obligation line, read as the Python pin read it.
/// The two leading letters are ASCII capitals, and the number
/// after the -S is one digit in the oracle's digit class,
/// which is the Unicode decimal class and not ASCII alone.
fn stub_obligation(line: &str) -> Option<(String, u32, String)> {
    if line.len() <= 6 {
        return None;
    }
    let chars: Vec<char> = line.chars().take(6).collect();
    if chars.len() < 6
        || !chars[0].is_ascii_uppercase()
        || !chars[1].is_ascii_uppercase()
        || chars[2] != '-'
        || chars[3] != 'S'
        || chars[5] != '.'
    {
        return None;
    }
    let number = decimal_value(chars[4] as u32)?;
    Some((chars[..2].iter().collect(), number, line.to_string()))
}

#[test]
fn the_contract_stub_obligations_exist() {
    let text = read("CONTRACT.md");
    let mut found: Vec<(String, u32, String)> = Vec::new();
    for line in text.split('\n') {
        if let Some(obligation) = stub_obligation(line) {
            found.push(obligation);
        }
    }
    let texts: Vec<&str> = found.iter().map(|(_, _, line)| line.as_str()).collect();
    assert_eq!(
        texts, EXPECTED_OBLIGATIONS,
        "stub obligation text has changed without a contract amendment"
    );
    let numbers: Vec<u32> = found.iter().map(|(_, n, _)| *n).collect();
    assert_eq!(
        numbers,
        [1, 2, 3, 4, 5],
        "stub obligations must be exactly S1 to S5"
    );
    for (prefix, _, line) in &found {
        assert_eq!(prefix, "ZH", "obligation is not a ZH obligation");
        let words = line.split_whitespace().count();
        assert!(words >= 20, "obligation too thin at {words} words");
    }
}

#[test]
fn the_agents_canonical_block_is_present() {
    let text = read("AGENTS.md");
    assert!(text.contains("repo annex below this line"));
    let prefix = text.split("## Repo annex").next().expect("prefix");
    let digest = format!("{:x}", Sha256::digest(prefix.as_bytes()));
    assert_eq!(
        digest, CANONICAL_PREFIX_SHA256,
        "canonical block above the annex line has drifted"
    );
    let annex = text.split("## Repo annex").nth(1).expect("annex");
    assert!(annex.contains("CONTRACT.md"));
}

#[test]
fn the_readme_status_reports_the_round_count() {
    let text = read("README.md");
    let counts = round_counts(&text);
    assert_eq!(
        counts,
        ["4"],
        "README status must report exactly one closed round count and it must be 4 of 100"
    );
}

/// Every digit run in a "Closed rounds N of 100" phrase whose
/// closing count is not followed by another digit. This is
/// the findall pin of the Python form, written without a
/// regex engine. Its digit class is the oracle's decimal
/// class, the Unicode decimal digits, as the Python pattern's
/// digit class was.
fn round_counts(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("Closed rounds ") {
        let after = &rest[start + "Closed rounds ".len()..];
        let digits: String = after
            .chars()
            .take_while(|c| is_decimal(*c as u32))
            .collect();
        if !digits.is_empty() {
            let tail = &after[digits.len()..];
            if let Some(tail) = tail.strip_prefix(" of 100") {
                let followed_by_digit = tail.chars().next().is_some_and(|c| is_decimal(c as u32));
                if !followed_by_digit {
                    found.push(digits.clone());
                }
            }
        }
        rest = &rest[start + 1..];
    }
    found
}

#[test]
fn the_setup_digit_class_is_the_unicode_decimal_class() {
    // The Python pins read digits in the decimal class of the
    // oracle's text patterns, which reaches every Unicode
    // decimal digit and nothing wider. An Arabic-Indic count
    // is a count. A Roman numeral is numeric in the standard
    // library's wider class but is not a decimal digit, and
    // neither pin may read it as one.
    assert_eq!(round_counts("Closed rounds \u{664} of 100"), ["\u{664}"]);
    assert!(round_counts("Closed rounds 4 of 100\u{664}").is_empty());
    let arabic = "ZH-S\u{664}. Every suspect on the hunt list carries.";
    assert_eq!(stub_obligation(arabic).map(|(_, n, _)| n), Some(4));
    let roman = "ZH-S\u{2163}. Every suspect on the hunt list carries.";
    assert_eq!(stub_obligation(roman), None);
}

#[test]
fn the_ci_backstop_step_runs_the_cargo_gate() {
    let text = read(".github/workflows/ci.yml");
    assert!(
        text.contains("Run CI backstop"),
        "the CI backstop step is missing"
    );
    assert!(
        text.contains("cargo test"),
        "the CI backstop must run the cargo gate"
    );
    assert!(
        text.contains("dtolnay/rust-toolchain"),
        "the CI gate needs the Rust toolchain step"
    );
    assert!(
        !text.contains("pytest"),
        "the Python gate is retired in this slice"
    );
}

const TEXT_SUFFIXES: [&str; 9] = [
    ".md", ".yml", ".yaml", ".rs", ".toml", ".txt", ".json", ".jsonl", ".lock",
];
const ANY_DEPTH_SKIP: [&str; 4] = [".git", "target", ".pytest_cache", "__pycache__"];
const ROOT_SKIP: [&str; 4] = [".venv", "venv", "node_modules", ".tox"];

fn collect_scanned(dir: &Path, depth: usize, out: &mut BTreeSet<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read dir");
    for entry in entries {
        let entry = entry.expect("entry");
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            if ANY_DEPTH_SKIP.contains(&name.as_str()) {
                continue;
            }
            if depth == 0 && ROOT_SKIP.contains(&name.as_str()) {
                continue;
            }
            collect_scanned(&path, depth + 1, out);
        } else if file_type.is_file() {
            let scanned = match path.extension().and_then(|e| e.to_str()) {
                None => true,
                Some(ext) => TEXT_SUFFIXES.contains(&format!(".{ext}").as_str()),
            };
            if scanned {
                out.insert(path);
            }
        }
    }
}

#[test]
fn no_em_or_en_dash_in_text_files() {
    let root = root();
    let mut scanned = BTreeSet::new();
    collect_scanned(&root, 0, &mut scanned);
    for anchor in [
        "CONTRACT.md",
        "Cargo.toml",
        "Cargo.lock",
        "src/lib.rs",
        "src/main.rs",
    ] {
        assert!(
            scanned.contains(&root.join(anchor)),
            "anchor file {anchor} was not scanned"
        );
    }
    let em = "\u{2014}".as_bytes();
    let en = "\u{2013}".as_bytes();
    let mut bad = Vec::new();
    for path in &scanned {
        let bytes = std::fs::read(path).expect("read scanned file");
        if bytes.windows(3).any(|w| w == em || w == en) {
            bad.push(
                path.strip_prefix(&root)
                    .expect("under root")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    assert!(bad.is_empty(), "em or en dash found in {bad:?}");
}
