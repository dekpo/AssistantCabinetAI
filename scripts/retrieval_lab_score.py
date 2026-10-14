"""Score what the retrieval lab recorded against the exam's ground truth.

Measurement tooling for `docs/RETRIEVAL-LAB.md`, not product code. It reads two things:

- `fixtures/knowledge-sandbox/questions.json`, the ground truth (`relevant_readable` is the list of files that
  hold the answer and that the product can read);
- the `run.jsonl` written by `apps/desktop/src-tauri/tests/retrieval_lab.rs`: for every question and channel, the
  ranked passages (path, page, whether the passage carries the planted fact) and the time.

For each channel it prints recall@5/10/30, MRR, nDCG@10, the wrong files, the fact found at k, and the time.
Recall is the share of the relevant FILES that appear among the first k PASSAGES. A passage whose file is not
relevant is a wrong passage; the distinct wrong files are counted. Questions with no relevant file (the
"nothing in the documents" questions) are scored apart: the number of passages returned, because a retrieval
that returns six passages for a question the documents cannot answer is what the refusal rule must catch.

    python scripts/retrieval_lab_score.py data/retrieval-lab/baseline
    python scripts/retrieval_lab_score.py data/retrieval-lab/baseline --detail Q010 Q012
    python scripts/retrieval_lab_score.py data/retrieval-lab/after --against data/retrieval-lab/baseline
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from pathlib import Path

CUTOFFS = (5, 10, 30)
DEFAULT_QUESTIONS = Path(__file__).resolve().parent.parent / "fixtures" / "knowledge-sandbox" / "questions.json"


def load_questions(path: Path) -> dict[str, dict]:
    return {q["id"]: q for q in json.loads(path.read_text(encoding="utf-8"))}


def load_run(folder: Path) -> dict[str, dict[str, dict]]:
    """channel -> question id -> the recorded line."""
    run: dict[str, dict[str, dict]] = {}
    text = (folder / "run.jsonl").read_text(encoding="utf-8")
    for raw in text.splitlines():
        if not raw.strip():
            continue
        row = json.loads(raw)
        run.setdefault(row["channel"], {})[row["id"]] = row
    return run


def first_ranks(results: list[dict], relevant: set[str]) -> list[int]:
    """Ranks (1-based, in passages) of the first passage of each relevant file."""
    seen: set[str] = set()
    ranks: list[int] = []
    for position, passage in enumerate(results, start=1):
        path = passage["path"]
        if path in relevant and path not in seen:
            seen.add(path)
            ranks.append(position)
    return ranks


def score_question(results: list[dict], relevant: set[str], facts_expected: bool, fact_pages: list[int] | None = None) -> dict:
    """The figures of one question that has at least one relevant file."""
    ranks = first_ranks(results, relevant)
    figures: dict = {"expected": len(relevant), "returned": len(results)}
    for k in CUTOFFS:
        figures[f"recall@{k}"] = sum(1 for r in ranks if r <= k) / len(relevant)
    figures["mrr"] = 1.0 / ranks[0] if ranks else 0.0
    gains = [1.0 if position in ranks else 0.0 for position in range(1, 11)]
    dcg = sum(gain / math.log2(position + 1) for position, gain in enumerate(gains, start=1))
    ideal = sum(1.0 / math.log2(position + 1) for position in range(1, min(len(relevant), 10) + 1))
    figures["ndcg@10"] = dcg / ideal if ideal else 0.0
    figures["hits"] = len(ranks)
    figures["wrong_files"] = len({p["path"] for p in results if p["path"] not in relevant})
    if facts_expected:
        for k in CUTOFFS:
            # a fact counts only in a passage of a RELEVANT file: templated documents repeat the same figures
            figures[f"fact@{k}"] = 1.0 if any(p.get("fact") and p["path"] in relevant for p in results[:k]) else 0.0
        if fact_pages:
            # the passage carries the fact AND sits on the page the ground truth gives (PDF; a DOCX is one page)
            figures["page@10"] = 1.0 if any(p.get("fact") and p["path"] in relevant and p["page"] in fact_pages for p in results[:10]) else 0.0
    return figures


def mean(values: list[float]) -> float | None:
    return statistics.fmean(values) if values else None


def summarise_channel(questions: dict[str, dict], rows: dict[str, dict]) -> dict:
    scored: dict[str, dict] = {}
    unanswerable: list[int] = []
    times: list[float] = []
    for qid, row in rows.items():
        question = questions.get(qid)
        if question is None:
            continue
        times.append(float(row["ms"]))
        relevant = set(question["relevant_readable"])
        if relevant:
            scored[qid] = score_question(row["results"], relevant, bool(question["expect"]["contains"]),
                                         question.get("fact_pages"))
        elif question["category"] in ("unknown", "entity_free") or not question["relevant_documents"]:
            unanswerable.append(len(row["results"]))
    summary: dict = {"questions": len(scored), "unanswerable": len(unanswerable)}
    for key in [f"recall@{k}" for k in CUTOFFS] + ["mrr", "ndcg@10"]:
        summary[key] = mean([s[key] for s in scored.values()])
    summary["questions_with_wrong_files"] = sum(1 for s in scored.values() if s["wrong_files"])
    summary["wrong_files_total"] = sum(s["wrong_files"] for s in scored.values())
    for k in CUTOFFS:
        summary[f"fact@{k}"] = mean([s[f"fact@{k}"] for s in scored.values() if f"fact@{k}" in s])
    summary["page@10"] = mean([s["page@10"] for s in scored.values() if "page@10" in s])
    summary["fact_questions"] = sum(1 for s in scored.values() if "fact@5" in s)
    summary["files_expected"] = sum(s["expected"] for s in scored.values())
    summary["files_found"] = sum(s["hits"] for s in scored.values())
    summary["unanswerable_passages"] = mean([float(n) for n in unanswerable])
    summary["ms_median"] = statistics.median(times) if times else None
    summary["ms_p95"] = sorted(times)[int(0.95 * (len(times) - 1))] if times else None
    summary["per_question"] = scored
    return summary


def by_category(questions: dict[str, dict], summary: dict) -> dict[str, dict]:
    groups: dict[str, list[dict]] = {}
    for qid, figures in summary["per_question"].items():
        groups.setdefault(questions[qid]["category"], []).append(figures)
    return {
        category: {
            "n": len(items),
            "recall@10": mean([i["recall@10"] for i in items]),
            "recall@30": mean([i["recall@30"] for i in items]),
            "mrr": mean([i["mrr"] for i in items]),
        }
        for category, items in sorted(groups.items())
    }


def fmt(value: float | None, digits: int = 3) -> str:
    return "-" if value is None else f"{value:.{digits}f}"


def print_table(summaries: dict[str, dict]) -> None:
    header = ["channel", "n", "R@5", "R@10", "R@30", "MRR", "nDCG@10", "fact@5", "fact@10", "fact@30", "page@10",
              "wrong files", "files found", "ms med", "ms p95"]
    print(" | ".join(header))
    print(" | ".join("---" for _ in header))
    for channel, s in summaries.items():
        print(" | ".join([
            channel, str(s["questions"]), fmt(s["recall@5"]), fmt(s["recall@10"]), fmt(s["recall@30"]),
            fmt(s["mrr"]), fmt(s["ndcg@10"]), fmt(s["fact@5"]), fmt(s["fact@10"]), fmt(s["fact@30"]), fmt(s["page@10"]),
            f"{s['wrong_files_total']} in {s['questions_with_wrong_files']} q",
            f"{s['files_found']}/{s['files_expected']}", fmt(s["ms_median"], 1), fmt(s["ms_p95"], 1),
        ]))


def set_sentences(questions: dict[str, dict], run: dict[str, dict[str, dict]]) -> list[str]:
    """The six set sentences: expected, returned by the word search and by the pipeline, wrong files."""
    lines = ["| id | sentence | expected | pipeline (6 passages) | hybrid30 | word search | wrong (word search) |",
             "| --- | --- | --- | --- | --- | --- | --- |"]
    totals = [0, 0, 0, 0, 0]
    for qid, row in sorted(run.get("term_all", {}).items()):
        question = questions[qid]
        relevant = set(question["relevant_readable"])
        found = {p["path"] for p in row["results"]}
        piped = {p["path"] for p in run.get("pipeline", {}).get(qid, {"results": []})["results"]}
        deep = {p["path"] for p in run.get("hybrid30", {}).get(qid, {"results": []})["results"]}
        wrong = found - relevant
        counts = [len(relevant), len(piped & relevant), len(deep & relevant), len(found & relevant), len(wrong)]
        totals = [a + b for a, b in zip(totals, counts)]
        lines.append(f"| {qid} | {question['text']} | {counts[0]} | {counts[1]} | {counts[2]} | {counts[3]} | {counts[4]} |")
    lines.append(f"| | total | {totals[0]} | {totals[1]} | {totals[2]} | {totals[3]} | {totals[4]} |")
    return lines


def detail(questions: dict[str, dict], run: dict[str, dict[str, dict]], ids: list[str]) -> None:
    for qid in ids:
        question = questions[qid]
        relevant = set(question["relevant_readable"])
        print(f"\n{qid} [{question['category']}] {question['text']}  (expected {len(relevant)} files)")
        for channel, rows in run.items():
            row = rows.get(qid)
            if row is None or channel == "embed":
                continue
            files = []
            for passage in row["results"]:
                if passage["path"] not in files:
                    files.append(passage["path"])
            hits = [f for f in files if f in relevant]
            wrong = [f for f in files if f not in relevant]
            print(f"  {channel}: {len(row['results'])} passages, {len(files)} files, {len(hits)} right, {len(wrong)} wrong,"
                  f" first right at {(first_ranks(row['results'], relevant) or ['-'])[0]}, {row['ms']:.1f} ms")
            for path in wrong[:3]:
                print(f"      wrong: {path}")


def compare(base: dict[str, dict], other: dict[str, dict]) -> None:
    print("\nchange against the other run (this - other):")
    for channel in other:
        if channel not in base:
            continue
        deltas = []
        for key in ("recall@10", "recall@30", "mrr", "ndcg@10", "fact@10"):
            a, b = base[channel][key], other[channel][key]
            deltas.append(f"{key} {'-' if a is None or b is None else f'{a - b:+.3f}'}")
        regressed = [
            qid for qid, f in base[channel]["per_question"].items()
            if qid in other[channel]["per_question"] and f["recall@10"] < other[channel]["per_question"][qid]["recall@10"] - 1e-9
        ]
        print(f"  {channel}: " + ", ".join(deltas) + f"; questions that lost recall@10: {len(regressed)} {regressed[:12]}")


def main(argv: list[str] | None = None) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("run", type=Path, help="folder holding run.jsonl")
    parser.add_argument("--questions", type=Path, default=DEFAULT_QUESTIONS)
    parser.add_argument("--against", type=Path, help="another run folder to compare with")
    parser.add_argument("--detail", nargs="*", help="question ids to print channel by channel")
    parser.add_argument("--categories", action="store_true", help="also print the figures per category")
    parser.add_argument("--json", type=Path, help="write the summaries to this file")
    args = parser.parse_args(argv)

    questions = load_questions(args.questions)
    run = load_run(args.run)
    summaries = {c: summarise_channel(questions, rows) for c, rows in run.items() if c not in ("embed", "term_all")}
    print_table(summaries)
    embeds = [float(r["ms"]) for r in run.get("embed", {}).values()]
    if embeds:
        print(f"\nquestion embedding: median {statistics.median(embeds):.1f} ms over {len(embeds)} questions")
    for channel, s in summaries.items():
        if s["unanswerable_passages"] is not None:
            print(f"{channel}: {s['unanswerable']} questions the documents cannot answer, "
                  f"{s['unanswerable_passages']:.1f} passages returned on average")
    if "term_all" in run:
        print("\nthe six set sentences:\n")
        print("\n".join(set_sentences(questions, run)))
    if args.categories:
        for channel, s in summaries.items():
            print(f"\nper category, {channel}:")
            for category, g in by_category(questions, s).items():
                print(f"  {category:15} n={g['n']:3} R@10 {fmt(g['recall@10'])} R@30 {fmt(g['recall@30'])} MRR {fmt(g['mrr'])}")
    if args.detail:
        detail(questions, run, args.detail)
    if args.against:
        other = {c: summarise_channel(questions, rows) for c, rows in load_run(args.against).items()
                 if c not in ("embed", "term_all")}
        compare(summaries, other)
    if args.json:
        args.json.write_text(json.dumps(summaries, ensure_ascii=False, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
