"""A read-only look at what the knowledge base measured for the questions asked (lot 6).

Prints, from the application's own index (opened read-only, so it changes nothing and migrates
nothing), the rows of kb_diagnostics: one row per question that went through the knowledge base, with
**counts, codes and milliseconds only** - never a question, a name or a file name - and the totals.
It also prints the last lines of retrieval-timings.jsonl (the same numbers, when "writeTimingLog" is
on), reduced to their knowledge fields.

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\show_kb_diagnostics.py
    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\show_kb_diagnostics.py path\\to\\index.sqlite3

Used by the human test of lot 6 (docs/test-reports/knowledge-base-pass-1/human-tests/lot-06-targeted-scope.md).
"""

import json
import os
import sqlite3
import sys
from pathlib import Path

APP_FOLDER = "com.assistantcabinetai.desktop"
LAST_LINES = 8


def data_dir() -> Path:
    override = os.environ.get("ACAI_DATA_DIR")
    if override:
        return Path(override)
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / APP_FOLDER
    return Path.home() / "Library" / "Application Support" / APP_FOLDER


def main(argv: list[str]) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    index = Path(argv[1]) if len(argv) > 1 else data_dir() / "index.sqlite3"
    if not index.is_file():
        print(f"No index at {index}")
        return 2
    connection = sqlite3.connect(f"{index.as_uri()}?mode=ro", uri=True)
    tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type = 'table'")}
    if "kb_diagnostics" not in tables:
        print("No kb_diagnostics table: this index was never opened by a build that has lot 6.")
        return 0
    columns = [row[1] for row in connection.execute("PRAGMA table_info(kb_diagnostics)")]
    if "would_apply" not in columns:
        print("kb_diagnostics exists but has no lot 6 columns yet: ask one question first.")
        return 0

    rows = connection.execute(
        "SELECT id, mode, path, strategy, IFNULL(skipped_reason, '-'), selected_files, candidate_files, "
        "selected_data, candidate_data, cited_files, cited_in_candidates, entities_found, widened, "
        "applied, would_apply, reduction_percent, miss_no_mention, miss_capped, miss_unresolved, "
        "knowledge_ms, retrieval_ms, total_ms FROM kb_diagnostics ORDER BY id"
    ).fetchall()
    print(f"Questions recorded: {len(rows)}   (counts, codes and milliseconds only)\n")
    header = (
        f"{'#':>3} {'mode':<8} {'path':<10} {'strategy':<13} {'skipped because':<26} "
        f"{'docs':>4} {'kept':>4} {'wbk':>3} {'kept':>4} {'cited':>5} {'in set':>6} {'names':>5} "
        f"{'widened':>7} {'applied':>7} {'would':>5} {'cut%':>4} {'know ms':>7}"
    )
    print(header)
    print("-" * len(header))
    for row in rows:
        (identifier, mode, path, strategy, skipped, docs, kept, wbk, wbk_kept, cited, in_set, names,
         widened, applied, would, cut, miss_none, miss_cap, miss_other, know_ms, _ret_ms, _tot_ms) = row
        print(
            f"{identifier:>3} {mode:<8} {path:<10} {strategy:<13} {skipped:<26} {docs:>4} {kept:>4} "
            f"{wbk:>3} {wbk_kept:>4} {cited:>5} {in_set:>6} {names:>5} {widened:>7} {applied:>7} "
            f"{would:>5} {cut:>4} {know_ms:>7}"
        )
    if rows:
        print(
            "\nLegend: docs = documents selected, kept = documents the reduced set holds (0 when "
            "nothing was reduced), wbk = workbooks selected, cited = files the answer cited, in set = "
            "of those, how many the reduced set held, names = entities found in the question, "
            "applied = the search really used the reduced set, would = a reduced set existed."
        )

    # The log sits beside the index, whichever index was asked for.
    log = index.parent / "retrieval-timings.jsonl"
    print(f"\nTiming log: {log}")
    if not log.is_file():
        print("  (absent: \"writeTimingLog\" is off or no question was asked)")
        return 0
    lines = log.read_text(encoding="utf-8").splitlines()
    print(f"  {len(lines)} line(s); the last {min(LAST_LINES, len(lines))}, knowledge fields only:")
    for line in lines[-LAST_LINES:]:
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            print("  (a line that is not JSON)")
            continue
        part = entry.get("targeting")
        if not part:
            print(f"  path={entry.get('path')}  (the knowledge base took no part)")
            continue
        print(
            f"  path={entry.get('path')} mode={part.get('mode')} outcome={part.get('outcome')} "
            f"skipped={part.get('skippedReason')} candidates={part.get('candidateDocuments')} "
            f"workbooks={part.get('candidateWorkbooks')} cut={part.get('reductionPercent')}% "
            f"widened={part.get('widened')} lookupMs={part.get('knowledgeLookupMs')} "
            f"entities={len(part.get('queryEntities', []))}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
