#!/usr/bin/env python3
"""
Nyx Spec / Docs Drift & Coverage Report Generator
=================================================

Purpose:
  Detects drift between the v1.0 protocol spec and the published delta docs
  (docs/*/v1_0_diff.md). Generates:
    1. JSON machine-readable report: spec/spec_diff_report.json
    2. Markdown human report: docs/spec_diff_report.md

What it measures:
  * Section hashes (SHA256) of v1.0 spec (English file) for change detection.
  * Presence/absence of each top-level (##) section in the delta docs via
    keyword matching.
  * Feature keyword coverage metrics per category (Cryptography, Routing, etc.).
  * Newly added / removed sections since previous run (tracked via prior JSON).

Idempotency:
  Re-running updates hashes; changed sections flagged.

Dependencies: standard library only.

Usage:
  python scripts/spec_diff.py            # from repo root

Exit Codes:
  0 success
  1 fatal error

Future extensions (not yet implemented but scaffold ready):
  * Mapping test files to spec sections via inline tags.
  * Multi-language spec comparison.
  * CI integration (non-zero exit if coverage drops below threshold).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from dataclasses import dataclass, asdict
from datetime import datetime, UTC
from pathlib import Path
from typing import Dict, List, Tuple, Optional

ROOT = Path(__file__).resolve().parent.parent
SPEC_DIR = ROOT / "spec"
DOCS_DIR = ROOT / "docs"
SPEC_FILE = SPEC_DIR / "Nyx_Protocol_v1.0_Spec_EN.md"
DELTA_DOC_EN = DOCS_DIR / "en" / "v1_0_diff.md"
DELTA_DOC_JA = DOCS_DIR / "ja" / "v1_0_diff.md"
JSON_REPORT = SPEC_DIR / "spec_diff_report.json"
MD_REPORT = DOCS_DIR / "spec_diff_report.md"

SECTION_HEADING_RE = re.compile(r"^## +(.+?)\s*$")

# Feature category → representative keywords to look for in diff docs.
FEATURE_KEYWORDS = {
    "Cryptography": ["HPKE", "Hybrid", "Kyber", "PQ", "Post-Quantum"],
    "Routing": ["Multipath", "WRR", "routing", "hop"],
    "Transport": ["QUIC", "TCP", "Teredo", "DATAGRAM"],
    "FEC": ["RaptorQ", "RS(255,223)", "redundancy"],
    "Plugin": ["Plugin", "Frame", "Capability", "CBOR"],
    "Telemetry": ["Telemetry", "Prometheus", "OpenTelemetry", "metrics"],
    # Include hyphenated and alternate forms for low power mode.
    "Low Power": ["Low Power", "Low-Power", "cover", "battery"],
    "Compliance": ["Compliance", "Core", "Plus", "Full"],
}


@dataclass
class SectionInfo:
    title: str
    hash: str
    changed: bool = False


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ""


def extract_sections(spec_content: str) -> List[Tuple[str, str]]:
    """Return list of (title, body) for each top-level '## ' section."""
    lines = spec_content.splitlines()
    sections: List[Tuple[str, List[str]]] = []
    current_title: Optional[str] = None
    current_body: List[str] = []
    for line in lines:
        m = SECTION_HEADING_RE.match(line)
        if m:
            if current_title is not None:
                sections.append((current_title, current_body))
            current_title = m.group(1).strip()
            current_body = []
        else:
            if current_title is not None:
                current_body.append(line)
    if current_title is not None:
        sections.append((current_title, current_body))
    out: List[Tuple[str, str]] = []
    for title, body_lines in sections:
        body = "\n".join(body_lines).strip()
        out.append((title, body))
    return out


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def build_section_infos(spec_content: str, previous: Dict[str, Dict]) -> List[SectionInfo]:
    infos: List[SectionInfo] = []
    for title, body in extract_sections(spec_content):
        h = sha256(body)
        prev_hash = previous.get(title, {}).get("hash") if previous else None
        infos.append(SectionInfo(title=title, hash=h, changed=(prev_hash is not None and prev_hash != h)))
    return infos


def _normalize_for_match(text: str) -> str:
    # Lowercase, replace hyphens/underscores with space, remove non-alphanum (keep spaces), collapse spaces.
    t = text.lower()
    t = re.sub(r"[-_]+", " ", t)
    t = re.sub(r"[^a-z0-9 ]+", " ", t)
    t = re.sub(r"\s+", " ", t).strip()
    return t


def keyword_coverage(doc_texts: List[str]) -> Dict[str, Dict[str, bool]]:
    joined = "\n".join(doc_texts)
    norm_text = _normalize_for_match(joined)
    cov: Dict[str, Dict[str, bool]] = {}
    for cat, kws in FEATURE_KEYWORDS.items():
        cov[cat] = {}
        for kw in kws:
            norm_kw = _normalize_for_match(kw)
            cov[cat][kw] = norm_kw in norm_text
    return cov


def coverage_score(cov: Dict[str, Dict[str, bool]]) -> float:
    total = 0
    present = 0
    for d in cov.values():
        for v in d.values():
            total += 1
            if v:
                present += 1
    return (present / total * 100.0) if total else 0.0


def load_previous_report() -> Dict:
    if JSON_REPORT.exists():
        try:
            return json.loads(JSON_REPORT.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def generate_markdown(sections: List[SectionInfo], cov: Dict[str, Dict[str, bool]], cov_score: float, new_sections: List[str], removed_sections: List[str], mapping_summary: Optional[Dict[str, object]] = None) -> str:
    lines: List[str] = []
    lines.append("# Spec / Docs Drift Report")
    lines.append("")
    lines.append(f"Generated: {datetime.now(UTC).isoformat()}")
    lines.append("")
    lines.append("## Section Hashes")
    lines.append("")
    lines.append("| Section | SHA256 | Changed |")
    lines.append("|---------|--------|---------|")
    for s in sections:
        lines.append(f"| {s.title} | `{s.hash[:12]}` | {'✅' if s.changed else ''} |")
    lines.append("")
    if new_sections or removed_sections:
        lines.append("## Structure Changes")
        if new_sections:
            lines.append("**New Sections:** " + ", ".join(new_sections))
        if removed_sections:
            lines.append("**Removed Sections:** " + ", ".join(removed_sections))
        lines.append("")
    lines.append("## Feature Keyword Coverage")
    lines.append("")
    lines.append(f"Overall Keyword Coverage: **{cov_score:.1f}%**")
    lines.append("")
    lines.append("| Category | Keyword | Present |")
    lines.append("|----------|---------|---------|")
    for cat, d in cov.items():
        for kw, present in d.items():
            lines.append(f"| {cat} | {kw} | {'✅' if present else '❌'} |")
    lines.append("")
    # Uncovered summary for quick gap-driven doc tasks.
    uncovered: List[str] = []
    for cat, d in cov.items():
        missing = [kw for kw, present in d.items() if not present]
        if missing:
            uncovered.append(f"- {cat}: {', '.join(missing)}")
    if uncovered:
        lines.append("### Uncovered Keywords")
        lines.append("")
        lines.append("以下のキーワードは diff ドキュメントで未検出です:")
        lines.extend(uncovered)
        lines.append("")
    lines.append("## Notes")
    lines.append("- 'Changed' indicates hash difference vs previous run for that section body.")
    lines.append("- Keyword coverage is heuristic (presence-based), not semantic validation.")

    # Section mapping coverage (from spec_test_mapping.json)
    if mapping_summary is not None:
        lines.append("")
        lines.append("## Section Mapping Coverage (@spec)")
        lines.append("")
        lines.append(f"Section Coverage: **{mapping_summary.get('section_coverage_percent', 0.0):.1f}%**  ")
        lines.append(f"Mapped Sections: {mapping_summary.get('mapped_section_count', 0)}/{mapping_summary.get('total_section_count', 0)}")
        unmapped = mapping_summary.get('unmapped_sections', []) or []
        if unmapped:
            lines.append("")
            lines.append("Unmapped Sections:")
            for s in unmapped:
                lines.append(f"- {s}")
        lines.append("")
    return "\n".join(lines) + "\n"


def main(threshold: float) -> int:
    spec_text = read_text(SPEC_FILE)
    if not spec_text:
        print(f"ERROR: Spec file not found: {SPEC_FILE}", file=sys.stderr)
        return 1

    prev = load_previous_report().get("sections", {})
    sections = build_section_infos(spec_text, prev)
    prev_titles = set(prev.keys()) if prev else set()
    current_titles = {s.title for s in sections}
    new_sections = sorted(list(current_titles - prev_titles))
    removed_sections = sorted(list(prev_titles - current_titles))

    # Coverage over both English & Japanese delta docs (if available)
    delta_docs = [read_text(DELTA_DOC_EN), read_text(DELTA_DOC_JA)]
    cov = keyword_coverage(delta_docs)
    cov_score = coverage_score(cov)

    # Optionally load mapping coverage from spec_test_mapping.json
    mapping_json_path = SPEC_DIR / "spec_test_mapping.json"
    mapping_data: Optional[Dict[str, object]] = None
    if mapping_json_path.exists():
        try:
            mapping_data = json.loads(mapping_json_path.read_text(encoding="utf-8"))
        except Exception:
            mapping_data = None

    report = {
        "timestamp": datetime.now(UTC).isoformat(),
        "spec_file": str(SPEC_FILE.relative_to(ROOT)),
        "sections": {s.title: asdict(s) for s in sections},
        "new_sections": new_sections,
        "removed_sections": removed_sections,
        "keyword_coverage_percent": cov_score,
        "keyword_coverage": cov,
        "uncovered_keywords": {cat: [kw for kw, present in d.items() if not present] for cat, d in cov.items() if any(not v for v in d.values())},
        # Mapping coverage passthrough if available
        "section_mapping": {
            "section_coverage_percent": float(mapping_data.get("section_coverage_percent", 0.0)) if mapping_data else None,
            "mapped_section_count": int(mapping_data.get("mapped_section_count", 0)) if mapping_data else None,
            "total_section_count": int(mapping_data.get("total_section_count", 0)) if mapping_data else None,
            "unmapped_sections": mapping_data.get("unmapped_sections", []) if mapping_data else None,
        },
        "coverage_threshold_met": cov_score >= threshold,
    }

    JSON_REPORT.write_text(json.dumps(report, indent=2), encoding="utf-8")
    MD_REPORT.write_text(
        generate_markdown(
            sections,
            cov,
            cov_score,
            new_sections,
            removed_sections,
            report.get("section_mapping") if report.get("section_mapping", {}).get("section_coverage_percent") is not None else None,
        ),
        encoding="utf-8",
    )

    print(f"Spec diff report written: {JSON_REPORT}")
    print(f"Markdown report written: {MD_REPORT}")
    if cov_score < threshold:
        print(f"WARNING: Coverage {cov_score:.1f}% below threshold {threshold:.1f}%", file=sys.stderr)
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate spec vs docs drift/coverage report.")
    parser.add_argument("--threshold", type=float, default=70.0, help="Minimum keyword coverage percent (warning only).")
    args = parser.parse_args()
    sys.exit(main(args.threshold))
