"""Score a run of the book. Deterministic where it can be (a number, a date, words, a list of files, a count, a
refusal, a road), and plain about what it cannot judge: a question marked `needs a reading` is scored on what can be
checked and left to a person for the rest, on a short sheet, never all 300.

    python scripts/large_test/score.py --library data/large-test/library --results data/large-test/2026-10-16-ministral

Reads `<results>/answers.jsonl` (or `routes.jsonl` with `--routes`), writes `scores.jsonl`, `summary.json`,
`report.md` and `reading-sheet.csv` beside it.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
import statistics
import sys
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from large_test import features as F  # noqa: E402

MONTHS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"]
REFUSAL_ERRORS = {"insufficient_evidence", "scope_unavailable"}
REFUSAL_PHRASES = [
    "ne contient pas", "ne contiennent pas", "ne mentionne", "ne mentionnent", "pas d'information", "aucune information",
    "aucun document", "ne trouve", "n'est pas", "ne sont pas", "pas précisé", "ne précise", "pas indiqué", "ne dit pas",
    "ne figure", "impossible de", "je n'ai pas", "n'ont pas", "introuvable", "pas mentionné", "pas dans les documents",
    "pas dans vos documents", "ne permet", "ne permettent", "ne répond", "ne porte", "pas trouvé", "sans réponse",
    "pas de réponse", "pas assez", "aucune mention", "aucun résultat", "ne semble", "ne semblent", "information absente",
    "not mentioned", "no information", "does not contain", "cannot find",
]


def fold(text: str) -> str:
    decomposed = unicodedata.normalize("NFD", text.replace("’", "'"))
    stripped = "".join(ch for ch in decomposed if not unicodedata.combining(ch))
    return re.sub(r"[^a-z0-9]+", " ", stripped.lower()).strip()


def numbers_in(text: str) -> list:
    """Every number the text writes, in each way it may be read (`1,234` is 1234 or 1.234)."""
    found = []
    for token in re.findall(r"-?\d[\d   '.,]*\d|-?\d", text):
        raw = token.replace(" ", "").replace(" ", "").replace(" ", "").replace("'", "")
        candidates = []
        if "," in raw and "." in raw:
            raw = raw.replace(".", "").replace(",", ".") if raw.rfind(",") > raw.rfind(".") else raw.replace(",", "")
            candidates.append(raw)
        elif "," in raw:
            head, _, tail = raw.rpartition(",")
            candidates.append(raw.replace(",", ""))
            if raw.count(",") == 1 or len(tail) != 3:
                candidates.append(f"{head.replace(',', '')}.{tail}")
        elif raw.count(".") > 1:
            candidates.append(raw.replace(".", ""))
        elif "." in raw:
            head, _, tail = raw.rpartition(".")
            candidates.append(raw)
            if len(tail) == 3:
                candidates.append(raw.replace(".", ""))
        else:
            candidates.append(raw)
        for candidate in candidates:
            try:
                found.append(float(candidate))
            except ValueError:
                pass
    return found


def dates_in(text: str) -> set:
    found = set()
    for day, month, year in re.findall(r"(\d{1,2})(?:er)? (" + "|".join(MONTHS) + r") (\d{4})", text.lower()):
        found.add(f"{year}-{MONTHS.index(month) + 1:02d}-{int(day):02d}")
    folded = fold(text)
    for day, month, year in re.findall(r"(\d{1,2})(?:er)? (" + "|".join(fold(m) for m in MONTHS) + r") (\d{4})", folded):
        found.add(f"{year}-{[fold(m) for m in MONTHS].index(month) + 1:02d}-{int(day):02d}")
    for day, month, year in re.findall(r"\b(\d{1,2})[/.](\d{1,2})[/.](\d{4})\b", text):
        found.add(f"{year}-{int(month):02d}-{int(day):02d}")
    found.update(re.findall(r"\b(\d{4}-\d{2}-\d{2})\b", text))
    return found


def listed_paths(structured) -> list:
    """The relative paths a listing answer names: the files it lists and the ones it folds."""
    out: list = []

    def walk(value, key=""):
        if isinstance(value, dict):
            for k, v in value.items():
                walk(v, k)
        elif isinstance(value, list):
            for v in value:
                walk(v, key)
        elif isinstance(value, str) and key in ("relativePath", "others", "path"):
            out.append(value)

    walk(structured)
    return out


def road_matches(expected: list, actual: str) -> bool:
    return any(actual == road or actual.startswith(road + ":") or (road == "error" and actual == "error") for road in expected)


class Judge:
    """One question against one replayed result."""

    def __init__(self, question: dict, replayed: dict):
        self.q = question
        self.r = replayed
        self.expect = question["expect"]
        self.structured = replayed.get("structured")
        self.model_text = replayed.get("answer") or ""
        self.deterministic_text = replayed.get("structured_text") or ""
        self.text = self.model_text if self.model_text else self.deterministic_text
        self.checks: dict = {}

    def check(self, name: str, ok: bool, detail: str = "") -> None:
        self.checks[name] = {"ok": bool(ok), "detail": detail}

    def run(self, with_answer: bool) -> dict:
        e, r = self.expect, self.r
        self.check("route", road_matches(self.q["route"], r.get("route", "")), f"{r.get('route')} (expected {' or '.join(self.q['route'])})")
        if with_answer is False and r.get("route", "").startswith(("global_retrieval", "every_document", "named_file", "mixed_tier", "without_documents")) or r.get("route", "").endswith("model_plan"):
            # A road recorded with no model: only the road and the sources can be judged.
            return self.sources_and_finish(partial=True)
        refused = r.get("error") in REFUSAL_ERRORS or any(p in fold(self.text) for p in (fold(x) for x in REFUSAL_PHRASES))
        if e.get("refusal"):
            invented = [p for p in e.get("must_not_patterns", []) if re.search(p, self.text, re.I)]
            self.check("refusal", refused and not invented, "refused" if refused else "answered where it should refuse")
        elif r.get("error") and r.get("error") not in REFUSAL_ERRORS and not e.get("nudge"):
            self.check("no_error", False, f"error {r['error']}")
        if e.get("facts"):
            folded = fold(self.text)
            missing = [g for g in e["facts"] if not any(fold(v) in folded for v in g)]
            self.check("facts", not missing, f"missing {missing}" if missing else "")
        if e.get("patterns"):
            missing = [p for p in e["patterns"] if not re.search(p, self.text, re.I)]
            self.check("patterns", not missing, f"missing {missing}" if missing else "")
        if e.get("value") is not None:
            tolerance = e.get("tolerance", 0.005)
            got = numbers_in(self.text)
            self.check("value", any(abs(n - e["value"]) <= tolerance + 1e-9 for n in got), f"expected {e['value']}, numbers {got[:6]}")
        if e.get("values"):
            got = numbers_in(self.text)
            missing = [v for v in e["values"] if not any(abs(n - v) <= e.get("tolerance", 0.005) + 1e-9 for n in got)]
            self.check("values", not missing, f"missing {missing}, numbers {got[:6]}" if missing else "")
        if e.get("date"):
            self.check("date", e["date"] in dates_in(self.text), f"expected {e['date']}, dates {sorted(dates_in(self.text))[:4]}")
        if e.get("must_not"):
            folded = fold(self.text)
            leaked = [s for s in e["must_not"] if fold(s) and fold(s) in folded]
            self.check("must_not", not leaked, f"leaked {leaked}" if leaked else "")
        if e.get("must_not_patterns") and not e.get("refusal"):
            invented = [p for p in e["must_not_patterns"] if re.search(p, self.text, re.I)]
            self.check("must_not_patterns", not invented, "")
        self.deterministic(e)
        return self.sources_and_finish(partial=False)

    def deterministic(self, e: dict) -> None:
        structured = self.structured if isinstance(self.structured, (dict, list)) else None
        if isinstance(structured, dict) and structured.get("kind") == "document_search_both":
            # Documents and workbooks, one list each: the documents are judged on the first, the workbooks on the second.
            workbooks = structured.get("workbooks")
            structured = structured.get("documents")
            if e.get("workbook_listing") is not None:
                found = set(listed_paths(workbooks))
                wanted = set(e["workbook_listing"])
                self.check("workbook_listing", found == wanted, f"workbooks {sorted(found)[:3]} (expected {sorted(wanted)[:3]})")
        if e.get("listing") is not None:
            paths = listed_paths(structured)
            expected, optional = set(e["listing"]), set(e.get("listing_optional", []))
            total = structured.get("total") if isinstance(structured, dict) else None
            wrong = [p for p in paths if p not in expected and p not in optional]
            ok_total = total is not None and len(expected) <= total <= len(expected) + len(optional)
            shown = len(set(paths) & expected)
            self.check("total", ok_total, f"total {total}, expected {len(expected)}")
            self.check("no_wrong_file", not wrong, f"{len(wrong)} wrong file(s) {wrong[:2]}")
            self.check("listing", shown >= min(len(expected), 50) - len(optional), f"{shown} of {len(expected)} expected files listed")
        if (e.get("count") is not None or e.get("count_range")) and not e.get("listing"):
            count = None
            if isinstance(structured, dict):
                count = structured.get("count", structured.get("total"))
            low, high = e.get("count_range") or [e["count"], e["count"]]
            self.check("count", count is not None and low <= count <= high, f"{count} (expected {low if low == high else f'{low} to {high}'})")
        if e.get("first"):
            ranking = structured.get("ranking", []) if isinstance(structured, dict) else []
            first = ranking[0].get("relativePath") if ranking else None
            self.check("first", first in e["first"], f"{first}")
        if e.get("columns"):
            folded = fold(self.deterministic_text)
            missing = [c for c in e["columns"] if fold(c) not in folded]
            self.check("columns", not missing, f"missing {missing}" if missing else "")
        if e.get("groups"):
            folded = fold(self.deterministic_text)
            got = numbers_in(self.deterministic_text)
            missing = [name for name, total in e["groups"].items() if fold(name) not in folded or not any(abs(n - total) <= 0.006 for n in got)]
            self.check("groups", not missing, f"missing {missing[:3]}" if missing else "")
        if e.get("ordered"):
            folded = fold(self.deterministic_text)
            positions = [folded.find(fold(name)) for name in e["ordered"]]
            self.check("ordered", all(p >= 0 for p in positions) and positions == sorted(positions), f"{positions}")
        if e.get("nudge"):
            kind = structured.get("kind") if isinstance(structured, dict) else None
            reason = structured.get("reason") if isinstance(structured, dict) else None
            self.check("nudge", e["nudge"] in (kind, reason), f"kind {kind}, reason {reason}")
        if e.get("template"):
            ok = isinstance(structured, dict) and structured.get("template") == e["template"] and structured.get("dataFile") == e["data_file"]
            self.check("plan", ok, f"{structured.get('template') if isinstance(structured, dict) else None}")

    def sources_and_finish(self, partial: bool) -> dict:
        e = self.expect
        cited = {s["path"] for s in self.r.get("sources", [])}
        if e.get("sources_any"):
            self.check("sources_any", bool(cited & set(e["sources_any"])), f"cited {sorted(cited)[:3]}")
        if e.get("sources_all"):
            self.check("sources_all", set(e["sources_all"]) <= cited, f"cited {sorted(cited)[:3]}")
        if e.get("page") and e.get("sources_any"):
            pages = {s["page"] for s in self.r.get("sources", []) if s["path"] in e["sources_any"]}
            self.check("page", e["page"] in pages, f"pages {sorted(pages)}")
        passed = all(c["ok"] for c in self.checks.values())
        return {
            "id": self.q["id"], "block": self.q["block"], "feature": self.q["feature"], "format": self.q["format"],
            "route": self.r.get("route"), "status": "pass" if passed else "fail", "needs_reading": self.q["scored"]["human"],
            "model_free": not self.r.get("generated"), "checks": self.checks,
            "total_ms": self.r.get("total_ms"), "generate_ms": self.r.get("generate_ms"),
            "unverified_numbers": self.r.get("unverified_numbers", []), "partial": partial,
        }


def percentile(values: list, p: float):
    if not values:
        return None
    values = sorted(values)
    return values[min(len(values) - 1, round(p * (len(values) - 1)))]


def summarise(scores: list, questions: list, meta: dict) -> dict:
    by = lambda key: defaultdict(lambda: [0, 0])
    blocks, features, formats = by(0), by(0), by(0)
    for s in scores:
        for table, key in ((blocks, s["block"]), (features, s["feature"]), (formats, s["format"] or "-")):
            table[key][1] += 1
            table[key][0] += s["status"] == "pass"
    refusals = [s for s, q in zip(scores, questions) if q["expect"].get("refusal")]
    leaks = [s for s, q in zip(scores, questions) if q["expect"].get("must_not") and not s["checks"].get("must_not", {"ok": True})["ok"]]
    wrong_files = [s for s in scores if not s["checks"].get("no_wrong_file", {"ok": True})["ok"]]
    times = [s["total_ms"] for s in scores if s["total_ms"] is not None]
    generated = [s["generate_ms"] for s in scores if s["generate_ms"]]
    return {
        **meta,
        "questions": len(scores), "passed": sum(1 for s in scores if s["status"] == "pass"),
        "blocks": {k: {"passed": v[0], "total": v[1]} for k, v in sorted(blocks.items())},
        "features": {k: {"passed": v[0], "total": v[1], "name": F.FEATURES[k][1]} for k, v in sorted(features.items())},
        "formats": {k: {"passed": v[0], "total": v[1]} for k, v in sorted(formats.items())},
        "refusals": {"correct": sum(1 for s in refusals if s["status"] == "pass"), "total": len(refusals)},
        "leaks": len(leaks), "wrong_files": len(wrong_files),
        "needs_reading": sum(1 for s in scores if s["needs_reading"]),
        "model_free": {"passed": sum(1 for s in scores if s["model_free"] and s["status"] == "pass"), "total": sum(1 for s in scores if s["model_free"])},
        "median_ms": statistics.median(times) if times else None, "p95_ms": percentile(times, 0.95),
        "median_generate_ms": statistics.median(generated) if generated else None, "p95_generate_ms": percentile(generated, 0.95),
        "unverified_numbers": sum(1 for s in scores if s["unverified_numbers"]),
    }


def render_report(summary: dict, scores: list, questions: list) -> str:
    lines = [f"# Result: {summary.get('model', '?')}", ""]
    for key in ("model", "embed_alias", "gateway", "temperature", "commit", "date", "machine", "seconds"):
        if summary.get(key) not in (None, ""):
            lines.append(f"- {key}: {summary[key]}")
    lines += ["", f"**{summary['passed']} of {summary['questions']} passed.**", ""]
    lines += ["| Block | Passed | Total |", "| --- | --- | --- |"]
    lines += [f"| {k} | {v['passed']} | {v['total']} |" for k, v in summary["blocks"].items()]
    lines += ["", "| Feature | Name | Passed | Total |", "| --- | --- | --- | --- |"]
    lines += [f"| {k} | {v['name']} | {v['passed']} | {v['total']} |" for k, v in summary["features"].items()]
    lines += ["", "| Format | Passed | Total |", "| --- | --- | --- |"]
    lines += [f"| {k} | {v['passed']} | {v['total']} |" for k, v in summary["formats"].items()]
    lines += [
        "", f"- Refusals correct: {summary['refusals']['correct']} of {summary['refusals']['total']}",
        f"- Leaks (a forbidden fact reached an answer): {summary['leaks']}",
        f"- Wrong files in a listing: {summary['wrong_files']}",
        f"- Answers by the engines alone (no model), passed: {summary['model_free']['passed']} of {summary['model_free']['total']}",
        f"- Questions that need a person's reading: {summary['needs_reading']}",
        f"- Answers with a figure that is in no source: {summary['unverified_numbers']}",
        f"- Time per question: median {fmt_ms(summary['median_ms'])}, 95th percentile {fmt_ms(summary['p95_ms'])}; the model's part: median {fmt_ms(summary['median_generate_ms'])}, 95th {fmt_ms(summary['p95_generate_ms'])}",
        "", "## Failures", "",
    ]
    by_id = {q["id"]: q for q in questions}
    for s in scores:
        if s["status"] == "fail":
            failed = "; ".join(f"{name}: {c['detail']}" for name, c in s["checks"].items() if not c["ok"])
            lines.append(f"- {s['id']} ({s['feature']}) {by_id[s['id']]['text']} - {failed}")
    return "\n".join(lines) + "\n"


def fmt_ms(value) -> str:
    return "-" if value is None else f"{value / 1000:.1f} s"


def reading_sheet(scores: list, questions: list, replays: dict) -> str:
    out = io.StringIO()
    writer = csv.writer(out, delimiter=";", lineterminator="\n")
    writer.writerow(["id", "question", "expected", "answer given", "program verdict", "your verdict (ok / wrong)", "remark"])
    from large_test.book import describe_expectation

    for s, q in zip(scores, questions):
        if s["needs_reading"]:
            answer = replays[q["id"]].get("answer") or replays[q["id"]].get("structured_text", "")
            writer.writerow([q["id"], q["text"], describe_expectation(q["expect"]), answer.replace("\n", " ")[:600], s["status"], "", ""])
    return out.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--routes", action="store_true", help="score routes.jsonl (no model) instead of answers.jsonl")
    args = parser.parse_args()
    name = "routes.jsonl" if args.routes else "answers.jsonl"
    questions = json.loads((args.library / "questions.json").read_text(encoding="utf-8"))
    replays = {}
    for line in (args.results / name).read_text(encoding="utf-8").splitlines():
        if line.strip():
            row = json.loads(line)
            replays[row["id"]] = row
    questions = [q for q in questions if q["id"] in replays]
    scores = [Judge(q, replays[q["id"]]).run(with_answer=not args.routes) for q in questions]
    meta_path = args.results / f"{name}.meta.json"
    meta = json.loads(meta_path.read_text(encoding="utf-8")) if meta_path.exists() else {}
    extra = args.results / "run.json"
    if extra.exists():
        meta.update(json.loads(extra.read_text(encoding="utf-8")))
    summary = summarise(scores, questions, meta)
    stem = "routes-" if args.routes else ""
    (args.results / f"{stem}scores.jsonl").write_text("\n".join(json.dumps(s, ensure_ascii=False) for s in scores) + "\n", encoding="utf-8")
    (args.results / f"{stem}summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (args.results / f"{stem}report.md").write_text(render_report(summary, scores, questions), encoding="utf-8")
    if not args.routes:
        (args.results / "reading-sheet.csv").write_text(reading_sheet(scores, questions, replays), encoding="utf-8")
    print(f"{summary['passed']} of {summary['questions']} passed")


if __name__ == "__main__":
    main()
