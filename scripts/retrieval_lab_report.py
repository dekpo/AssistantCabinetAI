"""A per-question report of retrieval-lab runs, for the owner to try the same questions in the application.

Measurement tooling, not product code. It reads the ground truth (`questions.json`) and one or more runs
(`run.jsonl` + `analyse.json`, written by `apps/desktop/src-tauri/tests/retrieval_lab.rs`) and writes one Markdown file:
the settings of each run, the totals, then every question with what it must find and, for each run, whether the
application's own path (the `pipeline` channel: the six passages it would cite) found it.

    python scripts/retrieval_lab_report.py --questions data/retrieval-lab-big/questions.json \\
        --run nomic=data/retrieval-lab/big-nomic --run bge-m3=data/retrieval-lab/big-bge-m3 \\
        --primary nomic --title "Big set" --out data/retrieval-lab/report-big.md

A cell reads `ok 2` (the planted fact is in the 2nd passage the application would cite), `no (deep 9)` (not in the
six, but in the 9th of the thirty the hybrid ranking reads), `no` (not in the thirty), `files 2/3` (a question with no
planted fact: the relevant files among the files cited), or `6 passages` (a question whose answer is absent from the
documents: how many passages the application would still cite).
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path

import retrieval_lab_score as score


def cited_files(results: list[dict]) -> list[str]:
    seen: list[str] = []
    for passage in results:
        if passage["path"] not in seen:
            seen.append(passage["path"])
    return seen


def fact_rank(results: list[dict], relevant: set[str]) -> int | None:
    for position, passage in enumerate(results, start=1):
        if passage.get("fact") and passage["path"] in relevant:
            return position
    return None


def cell(question: dict, pipeline: dict | None, deep: dict | None) -> str:
    if pipeline is None:
        return "-"
    relevant = set(question["relevant_readable"])
    results = pipeline["results"]
    if not relevant:
        return f"{len(results)} passages"
    if question["expect"]["contains"]:
        rank = fact_rank(results, relevant)
        if rank is not None:
            return f"ok {rank}"
        deep_rank = fact_rank(deep["results"], relevant) if deep else None
        return f"no (deep {deep_rank})" if deep_rank else "no"
    found = len(relevant & set(cited_files(results)))
    return f"files {found}/{len(relevant)}"


def short(path: str) -> str:
    return path.split("/")[-1]


def expected(question: dict) -> str:
    files = question["relevant_readable"]
    if not files:
        return "nothing: the documents do not answer"
    text = short(files[0]) if len(files) == 1 else f"{len(files)} files, e.g. {short(files[0])}"
    facts = question["expect"]["contains"]
    pages = question.get("fact_pages") or []
    if facts:
        text += " - " + " + ".join(f"`{f}`" for f in facts)
        if pages and files[0].endswith(".pdf"):
            text += f" (p. {', '.join(map(str, pages))})"
    return text


def escape(text: str) -> str:
    return text.replace("|", "\\|")


def load_run(label: str, folder: Path) -> dict:
    run = score.load_run(folder)
    analyse = json.loads((folder / "analyse.json").read_text(encoding="utf-8"))
    return {"label": label, "channels": run, "analyse": analyse}


def settings_table(runs: list[dict]) -> list[str]:
    lines = ["| Run | Model | Dim. | Documents | Passages | Analyse | Per passage | Index | Question embedding (median) |",
             "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for run in runs:
        a = run["analyse"]
        embed = [float(r["ms"]) for r in run["channels"].get("embed", {}).values()]
        timings = a.get("timings") or {}
        per = (timings.get("embedMs", 0) / 1000 / a["chunks"]) if a["chunks"] else 0
        lines.append(
            f"| {run['label']} | `{a['embedding_alias']}` | {a['dimension']} | {a['documents_read']} | {a['chunks']} | "
            f"{a['analyse_ms'] / 60000:.1f} min | {per:.2f} s | {a['index_bytes'] / 1e6:.1f} MB | "
            f"{statistics.median(embed) / 1000:.2f} s |" if embed else
            f"| {run['label']} | `{a['embedding_alias']}` | {a['dimension']} | {a['documents_read']} | {a['chunks']} | "
            f"{a['analyse_ms'] / 60000:.1f} min | {per:.2f} s | {a['index_bytes'] / 1e6:.1f} MB | - |")
    return lines


def totals_table(questions: dict, runs: list[dict]) -> list[str]:
    lines = ["| Run | Channel | Questions | R@5 | R@10 | R@30 | MRR | nDCG@10 | fact@5 | fact@10 | fact@30 |",
             "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"]
    for run in runs:
        for channel in ("pipeline", "hybrid30", "lexical30", "vector30"):
            if channel not in run["channels"]:
                continue
            s = score.summarise_channel(questions, run["channels"][channel])
            lines.append(f"| {run['label']} | {channel} | {s['questions']} | {score.fmt(s['recall@5'])} | "
                         f"{score.fmt(s['recall@10'])} | {score.fmt(s['recall@30'])} | {score.fmt(s['mrr'])} | "
                         f"{score.fmt(s['ndcg@10'])} | {score.fmt(s['fact@5'])} | {score.fmt(s['fact@10'])} | "
                         f"{score.fmt(s['fact@30'])} |")
    return lines


def category_table(questions: dict, runs: list[dict]) -> list[str]:
    summaries = {r["label"]: score.summarise_channel(questions, r["channels"]["pipeline"]) for r in runs}
    categories = sorted({q["category"] for q in questions.values()})
    header = "| Category | Questions | " + " | ".join(f"{r['label']} R@10 / fact in the cited passages" for r in runs) + " |"
    lines = [header, "| --- | --- | " + " | ".join("---" for _ in runs) + " |"]
    for category in categories:
        members = [qid for qid, q in questions.items() if q["category"] == category]
        cells = []
        for run in runs:
            per = summaries[run["label"]]["per_question"]
            scored = [per[i] for i in members if i in per]
            recall = score.mean([s["recall@10"] for s in scored])
            facts = score.mean([s["fact@10"] for s in scored if "fact@10" in s])
            cells.append(f"{score.fmt(recall, 2)} / {score.fmt(facts, 2)}")
        lines.append(f"| {category} | {len(members)} | " + " | ".join(cells) + " |")
    return lines


def question_tables(questions: dict, runs: list[dict], primary: dict) -> list[str]:
    lines: list[str] = []
    categories = sorted({q["category"] for q in questions.values()})
    for category in categories:
        members = [q for q in questions.values() if q["category"] == category]
        lines += ["", f"### {category} ({len(members)})", "",
                  "| Id | Question | Must find | " + " | ".join(r["label"] for r in runs) + " |",
                  "| --- | --- | --- | " + " | ".join("---" for _ in runs) + " |"]
        for question in members:
            cells = []
            for run in runs:
                pipeline = run["channels"].get("pipeline", {}).get(question["id"])
                deep = run["channels"].get("hybrid30", {}).get(question["id"])
                cells.append(cell(question, pipeline, deep))
            lines.append(f"| {question['id']} | {escape(question['text'])} | {escape(expected(question))} | "
                         + " | ".join(cells) + " |")
    return lines


def misses(questions: dict, primary: dict) -> list[str]:
    lines: list[str] = []
    pipeline = primary["channels"].get("pipeline", {})
    for qid, question in questions.items():
        row = pipeline.get(qid)
        if row is None:
            continue
        relevant = set(question["relevant_readable"])
        if not relevant:
            continue
        if question["expect"]["contains"]:
            failed = fact_rank(row["results"], relevant) is None
        else:
            failed = len(relevant & set(cited_files(row["results"]))) < len(relevant)
        if failed:
            files = cited_files(row["results"])
            lines.append(f"- **{qid}** {question['text']}  \n  must find: {expected(question)}  \n  "
                         f"the application would cite: " + (", ".join(f"`{short(f)}`" + ("" if f in relevant else " (wrong)")
                                                              for f in files[:6]) or "nothing"))
    return lines


def main(argv: list[str] | None = None) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--questions", type=Path, required=True)
    parser.add_argument("--run", action="append", required=True, help="label=folder, repeat for each run")
    parser.add_argument("--primary", required=True, help="the label of the run that matches the model in the application")
    parser.add_argument("--title", default="Retrieval lab report")
    parser.add_argument("--folder", default="", help="the folder of documents the questions were asked on")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)

    questions = score.load_questions(args.questions)
    runs = []
    for spec in args.run:
        label, _, folder = spec.partition("=")
        runs.append(load_run(label, Path(folder)))
    primary = next(r for r in runs if r["label"] == args.primary)

    out = [f"# {args.title}", "",
           "Generated by `scripts/retrieval_lab_report.py` from the runs below. Fictional documents only; the book extracts"
           " are the owner's own copies and stay outside git.", ""]
    if args.folder:
        out += [f"Documents folder the questions were asked on: `{args.folder}`.", ""]
    out += ["Read each cell as: `ok n` the planted fact is in the n-th of the (at most six) passages the application's own path"
            " would cite; `no (deep n)` not in those six but in the n-th of the thirty the hybrid ranking reads; `no` not found;"
            " `files a/b` the relevant files among the files cited (questions without a planted fact); `n passages` the"
            " passages still cited for a question the documents cannot answer (the model must then refuse).", "",
            "## The runs", ""] + settings_table(runs) + ["", "## Totals by channel", ""] + totals_table(questions, runs)
    out += ["", f"## By category (application path; `{args.primary}` is the model the application uses today)", ""]
    out += category_table(questions, runs)
    out += ["", f"## Questions the application path misses with `{args.primary}`", ""]
    out += misses(questions, primary) or ["None."]
    out += ["", "## Every question", ""] + question_tables(questions, runs, primary)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"{args.out}: {len(questions)} questions, {len(runs)} runs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
