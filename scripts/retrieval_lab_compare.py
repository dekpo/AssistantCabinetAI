"""Compare retrieval-lab runs family by family, the way a method is judged (`docs/RETRIEVAL-QUALITY.md`, section 4).

Measurement tooling, not product code. For each run against the base it prints, per channel, the change of
recall@10 and of fact@10 overall and in every family (a "family" is the category of the exam, or the kind of
planted fact with --planted), and lists the questions that got worse with their text. A method is kept only
if something rises and no family falls.

    python scripts/retrieval_lab_compare.py data/retrieval-lab/off data/retrieval-lab/phrases --channels pipeline lexical30
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import retrieval_lab_score as score

WORSE = -1e-9


def scored_questions(questions: dict[str, dict], rows: dict[str, dict]) -> dict[str, dict]:
    """question id -> the figures of that question (only the ones with a relevant file)."""
    scored = {}
    for qid, row in rows.items():
        question = questions.get(qid)
        if question is None:
            continue
        relevant = set(question["relevant_readable"])
        if not relevant:
            continue
        scored[qid] = score.score_question(
            row["results"], relevant, bool(question["expect"]["contains"]),
            question.get("fact_pages"), question.get("fact_sections"),
        )
    return scored


def spread(rows: dict[str, dict], questions: dict[str, dict], size: int = 6) -> float | None:
    """Mean number of distinct (file, section) among the first `size` passages: how widely the places are spread."""
    counts = []
    for qid, row in rows.items():
        if qid not in questions or not row["results"]:
            continue
        places = {(p["path"], p.get("section_label", f"p{p['page']}")) for p in row["results"][:size]}
        counts.append(len(places))
    return sum(counts) / len(counts) if counts else None


def family_of(question: dict, by: str) -> str:
    return question.get("plant_kind") or question.get("kind") or question["category"] if by == "planted" else question["category"]


def figure(figures: dict, name: str) -> float | None:
    return figures.get(name)


def compare_channel(questions, base_rows, other_rows, by, label):
    base = scored_questions(questions, base_rows)
    other = scored_questions(questions, other_rows)
    shared = [qid for qid in base if qid in other]
    lines = []
    worse_ids = []
    before, after = spread(base_rows, questions), spread(other_rows, questions)
    if before is not None and after is not None:
        lines.append(f"  {label} places spread (distinct file+section in the first 6): {before:.2f} -> {after:.2f}")
    for metric in ("recall@10", "fact@10"):
        values_base = [figure(base[q], metric) for q in shared if figure(base[q], metric) is not None]
        values_other = [figure(other[q], metric) for q in shared if figure(base[q], metric) is not None]
        if values_base:
            delta = sum(values_other) / len(values_other) - sum(values_base) / len(values_base)
            lines.append(f"  {label} {metric}: {sum(values_base)/len(values_base):.3f} -> {sum(values_other)/len(values_other):.3f} ({delta:+.3f}) over {len(values_base)} questions")
    families: dict[str, list[str]] = {}
    for qid in shared:
        families.setdefault(family_of(questions[qid], by), []).append(qid)
    for family, ids in sorted(families.items()):
        parts = []
        falls = False
        for metric in ("recall@10", "fact@10"):
            b = [figure(base[q], metric) for q in ids if figure(base[q], metric) is not None]
            o = [figure(other[q], metric) for q in ids if figure(base[q], metric) is not None]
            if b:
                delta = sum(o) / len(o) - sum(b) / len(b)
                if delta < WORSE:
                    falls = True
                if abs(delta) > 1e-9:
                    parts.append(f"{metric} {delta:+.3f}")
        if parts:
            lines.append(f"    {'WORSE ' if falls else 'better'} {family:<16} n={len(ids):<3} " + ", ".join(parts))
    for qid in shared:
        for metric in ("recall@10", "fact@10", "mrr"):
            b, o = figure(base[qid], metric), figure(other[qid], metric)
            if b is not None and o is not None and o < b - 1e-9:
                worse_ids.append((qid, metric, b, o))
    for qid, metric, b, o in worse_ids:
        lines.append(f"    worse {qid} [{questions[qid]['category']}] {metric} {b:.3f} -> {o:.3f}: {questions[qid]['text']}")
    return lines, worse_ids


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base", type=Path)
    parser.add_argument("others", type=Path, nargs="+")
    parser.add_argument("--questions", type=Path, default=score.DEFAULT_QUESTIONS)
    parser.add_argument("--channels", nargs="*", default=["pipeline", "lexical30"])
    parser.add_argument("--planted", action="store_true", help="families are the kinds of planted fact")
    args = parser.parse_args(argv)
    questions = score.load_questions(args.questions)
    base_run = score.load_run(args.base)
    for other in args.others:
        other_run = score.load_run(other)
        print(f"=== {other.name} against {args.base.name}")
        for channel in args.channels:
            if channel not in base_run or channel not in other_run:
                print(f"  (no channel {channel})")
                continue
            lines, _ = compare_channel(questions, base_run[channel], other_run[channel], "planted" if args.planted else "category", channel)
            print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
