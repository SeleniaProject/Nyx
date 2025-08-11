#!/usr/bin/env python3
"""Generate Spec-to-Test mapping from @spec annotations.

Scans Rust test files for lines beginning with '/// @spec ' and maps each
spec section (exact text after token) to test identifiers in the form
'<relative_path>::<fn_name>'. Output:
  - docs/SPEC_TEST_MAPPING.md (overwrites existing skeleton sections below marker)
  - spec/spec_test_mapping.json

Limitations:
  - Requires tests to use free functions with #[test] or #[tokio::test]
  - Does not parse nested modules deeply (simple regex)
"""
from __future__ import annotations
import re, json, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
TEST_GLOBS = ["**/tests/**/*.rs", "**/src/tests/**/*.rs"]
SPEC_FILE = ROOT/"spec"/"Nyx_Protocol_v1.0_Spec_EN.md"
JSON_OUT = ROOT/"spec"/"spec_test_mapping.json"
MD_OUT = ROOT/"docs"/"SPEC_TEST_MAPPING.md"
MARKER = "## 当面の手動ダイジェスト (抜粋)"  # we will replace everything below this with generated table

SPEC_SECTIONS = []
sec_re = re.compile(r'^## +(.+)$')
with SPEC_FILE.open(encoding='utf-8') as f:
    for line in f:
        m = sec_re.match(line.strip())
        if m:
            SPEC_SECTIONS.append(m.group(1).strip())

# Collect annotations
spec_map: dict[str, list[str]] = {}
fn_name_re = re.compile(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(')
annot_re = re.compile(r'^\s*///\s*@spec\s+(.+)$')
current_annots: list[str] = []
current_fn: str | None = None
current_path: Path | None = None

def flush():
    if current_fn and current_annots and current_path:
        ident = f"{current_path.relative_to(ROOT).as_posix()}::{current_fn}"
        for sec in current_annots:
            bucket = spec_map.setdefault(sec, [])
            if ident not in bucket:
                bucket.append(ident)

for glob in TEST_GLOBS:
    for path in ROOT.glob(glob):
        if not path.is_file():
            continue
        current_annots = []
        current_fn = None
        current_path = path
        with path.open(encoding='utf-8') as f:
            for line in f:
                a = annot_re.match(line)
                if a:
                    current_annots.append(a.group(1).strip())
                m = fn_name_re.match(line)
                if m:
                    # reached a function; previous annotations belong to this fn
                    current_fn = m.group(1)
                    flush()
                    current_annots = []
                    current_fn = None
        # in case file ends without new function (ignored)

# Determine unmapped sections (ignore those before numbered sections for now)
unmapped = [s for s in SPEC_SECTIONS if any(s.startswith(prefix) for prefix in ("1.","2.","3.","4.","5.","6.","7.","8.","9.","10.")) and s not in spec_map]

total_numbered = len([s for s in SPEC_SECTIONS if any(s.startswith(prefix) for prefix in ("1.","2.","3.","4.","5.","6.","7.","8.","9.","10."))])
section_coverage = (len(spec_map)/total_numbered*100.0) if total_numbered else 0.0
report = {"sections": spec_map, "unmapped_sections": unmapped, "section_coverage_percent": section_coverage, "mapped_section_count": len(spec_map), "total_section_count": total_numbered}
JSON_OUT.write_text(json.dumps(report, indent=2), encoding='utf-8')

# Update Markdown
if MD_OUT.exists():
    orig_lines = MD_OUT.read_text(encoding='utf-8').splitlines()
else:
    orig_lines = ["# Spec-to-Test Mapping (自動生成)\n", MARKER]

# Find marker
try:
    marker_idx = next(i for i,l in enumerate(orig_lines) if l.strip()==MARKER)
except StopIteration:
    # Append marker at end if missing
    orig_lines.append(MARKER)
    marker_idx = len(orig_lines)-1

kept = orig_lines[:marker_idx+1]

generated: list[str] = []
generated.append("")
generated.append(f"自動生成テーブル (セクションカバレッジ {section_coverage:.1f}%: {len(spec_map)}/{total_numbered}):")
generated.append("")
generated.append('| Spec 節 | テストケース |')
generated.append('|---------|--------------|')
for sec, tests in sorted(spec_map.items(), key=lambda x: x[0]):
    generated.append(f"| {sec} | {'<br>'.join(tests)} |")
generated.append("")
generated.append('未マッピング節: ' + (', '.join(unmapped) if unmapped else 'なし'))
generated.append("")
generated.append('---')
generated.append('このセクション以下は自動生成されます。手動編集は次回上書きされます。')

MD_OUT.write_text('\n'.join(kept+generated)+"\n", encoding='utf-8')
print(f"Wrote {JSON_OUT} and updated {MD_OUT}")
