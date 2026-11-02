import hashlib
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
CANONICAL_PREFIX_SHA256 = "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11"
EXPECTED_OBLIGATIONS = [
    'ZH-S1. Every suspect on the hunt list carries the evidence that it serves nobody, and a suspect listed without evidence is a defect the tests must catch.',
    'ZH-S2. No kill fires before its proof is recorded, and an action taken on a suspect whose proof is missing or failed is a defect the tests must catch.',
    'ZH-S3. The first action on any proven zombie is a reversible quarantine, and an irreversible action as the first step on any suspect is forbidden by the contract.',
    'ZH-S4. Every hunt works from recorded or synthetic cluster state during the first slices, and no code path in those slices connects to a live cluster.',
    'ZH-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.',
]
EXPECTED_PREFIX = "ZH"
TEXT_SUFFIXES = {".md", ".yml", ".yaml", ".py", ".toml", ".txt"}


def test_contract_stub_obligations_exist():
    text = (ROOT / "CONTRACT.md").read_text()
    found = []
    for ln in text.splitlines():
        m = re.match(r"^([A-Z]{2})-S\d\.", ln)
        if m:
            found.append((m.group(1), ln))
    numbers = sorted(int(re.search(r"-S(\d)\.", ln).group(1)) for _, ln in found)
    texts = [ln for _, ln in found]
    assert texts == EXPECTED_OBLIGATIONS, "stub obligation text has changed without a contract amendment"
    assert numbers == [1, 2, 3, 4, 5], f"stub obligations must be exactly S1 to S5, found {numbers}"
    for prefix, ln in found:
        assert prefix == EXPECTED_PREFIX, f"obligation {ln[:12]} is not a {EXPECTED_PREFIX} obligation"
        words = len(ln.split())
        assert words >= 20, f"obligation too thin at {words} words, {ln[:40]}"


def test_agents_canonical_block_present():
    text = (ROOT / "AGENTS.md").read_text()
    assert "repo annex below this line" in text
    prefix = text.split("## Repo annex", 1)[0]
    digest = hashlib.sha256(prefix.encode()).hexdigest()
    assert digest == CANONICAL_PREFIX_SHA256, "canonical block above the annex line has drifted"
    annex = text.split("## Repo annex", 1)[1]
    assert "CONTRACT.md" in annex


def test_readme_status_reports_rounds():
    text = (ROOT / "README.md").read_text()
    assert re.search(r"Closed rounds 0 of 100(?!\d)", text), "README status must report closed rounds 0 of 100 exactly"


def test_no_em_or_en_dash_in_text_files():
    bad = []
    for p in ROOT.rglob("*"):
        if not p.is_file() or p.suffix not in TEXT_SUFFIXES:
            continue
        rel_parts = p.relative_to(ROOT).parts
        if ".git" in rel_parts or any(part in {".pytest_cache", "__pycache__"} for part in rel_parts) or (rel_parts and rel_parts[0] in {".venv", "venv", "node_modules", ".tox"}):
            continue
        t = p.read_text()
        if "\u2014" in t or "\u2013" in t:
            bad.append(str(p.relative_to(ROOT)))
    assert not bad, f"em or en dash found in {bad}"
