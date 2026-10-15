"""Score the application's refusal on a retrieval-lab run (`docs/RETRIEVAL-QUALITY.md`, lot E).

Measurement tooling, not product code. The lab writes, for every question it replays, a `refusal` line: would the
application have refused the question before any embedding or model, because every name and code it holds is
unknown to the documents? Against the ground truth this gives the two figures that decide whether the check is
kept: the **false refusals** (a question the documents can answer, refused: must be 0) and the **true refusals**
(a question they cannot answer, refused: the higher the better).

    python scripts/retrieval_lab_refusal.py data/retrieval-lab/<run> --questions <root>/questions.json
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import retrieval_lab_score as score


def judge(questions: dict[str, dict], rows: dict[str, dict]) -> dict:
    """Split the refusal lines into answerable and unanswerable questions, and count what was refused."""
    answerable_refused: list[tuple[str, str]] = []
    unanswerable_refused: list[str] = []
    answerable = unanswerable = 0
    for qid, row in rows.items():
        question = questions.get(qid)
        if question is None:
            continue
        if question["relevant_readable"]:
            answerable += 1
            if row.get("refused"):
                answerable_refused.append((qid, question["text"]))
        elif question["category"] != "data":
            unanswerable += 1
            if row.get("refused"):
                unanswerable_refused.append(qid)
    return {
        "answerable": answerable,
        "answerable_refused": answerable_refused,
        "unanswerable": unanswerable,
        "unanswerable_refused": unanswerable_refused,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("run", type=Path)
    parser.add_argument("--questions", type=Path, default=score.DEFAULT_QUESTIONS)
    args = parser.parse_args(argv)
    run = score.load_run(args.run)
    if "refusal" not in run:
        print("this run has no refusal channel")
        return 1
    verdict = judge(score.load_questions(args.questions), run["refusal"])
    print(f"false refusals: {len(verdict['answerable_refused'])} of {verdict['answerable']} answerable questions")
    for qid, text in verdict["answerable_refused"]:
        print(f"  {qid}: {text}")
    print(f"true refusals: {len(verdict['unanswerable_refused'])} of {verdict['unanswerable']} unanswerable questions")
    return 0


if __name__ == "__main__":
    sys.exit(main())
