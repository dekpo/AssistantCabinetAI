"""The question book: 300 questions (150 on documents, 100 on spreadsheets, 50 that need both), each with the feature
it demonstrates, the files that hold the answer, what the answer must contain, the road the application is expected
to take, and what the scorer decides by itself.

Every expectation is computed from the generated data (the Python values the files were written from), never from
the files and never from the application. The roads are the author's reading of each feature; the harness records
the road really taken, and a question whose road differs is reviewed in the report, not silently adjusted
(`docs/test-reports/large-test/README.md`, section 5).

The book is built in the same process as the library, because it needs the text of every document and the rows of
every table.
"""

from __future__ import annotations

import csv
import io
import random
import re

from knowledge_sandbox.cast import fold

from . import features as F

# Questions per block: the full book, and the versioned slice.
QUOTAS = {"documents": 150, "spreadsheets": 100, "mixed": 50}
SLICE_QUOTAS = {"documents": 15, "spreadsheets": 10, "mixed": 6}

PREFIX = {"documents": "D", "spreadsheets": "T", "mixed": "M"}


def fold_text(text: str) -> str:
    """Lowercase, accents removed, anything that is not a letter or a digit read as a space, one space between
    words: the form words are compared in."""
    return fold(text)


def has_phrase(folded_text: str, phrase: str) -> bool:
    """Whether `phrase` is in the folded text as whole words."""
    wanted = fold_text(phrase)
    return bool(wanted) and f" {wanted} " in f" {folded_text} "


class Builder:
    def __init__(self, library, books, seed: int, slice_only: bool):
        self.library = library
        self.cast = library.cast
        self.records = library.records
        self.by_path = {r["path"]: r for r in self.records}
        self.books = books
        self.workbook_by_path = {b.path: b for b in books}
        self.slice = slice_only
        self.quota = SLICE_QUOTAS if slice_only else QUOTAS
        self.rng = random.Random(f"book-{seed}")
        self.questions: list = []
        self.feature_cap: dict = {}
        self.asked: set = set()
        self.feature_count: dict = {}
        self.folded = {path: fold_text(text) for path, text in library.texts.items()}

    # -------------------------------------------------------------------------------------------------- adding

    def count(self, block: str) -> int:
        return sum(1 for q in self.questions if q["block"] == block)

    def full(self, block: str) -> bool:
        return self.count(block) >= self.quota[block]

    def add(self, block: str, feature: str, text: str, *, expect: dict, route: list, selection: dict | None = None,
            documents: list | None = None, workbooks: list | None = None, human: bool = False,
            follow_up_of: str | None = None, note: str = "", fmt: str = "") -> dict | None:
        """One question. `documents` and `workbooks` are the files that hold the answer."""
        if self.full(block):
            return None
        assert F.block_of(feature) == block, (feature, block)
        if self.feature_cap and self.feature_count.get(feature, 0) >= self.feature_cap.get(feature, 0):
            return None
        key = (text, repr(selection), follow_up_of)
        if key in self.asked:
            return None
        self.asked.add(key)
        self.feature_count[feature] = self.feature_count.get(feature, 0) + 1
        number = self.count(block) + 1
        documents = documents or []
        indices = [self.by_path[p]["index"] for p in documents if p in self.by_path]
        question = {
            "id": f"{PREFIX[block]}-{number:03d}",
            "block": block,
            "feature": feature,
            "format": fmt,
            "text": text,
            "follow_up_of": follow_up_of,
            "selection": selection or default_selection(block),
            "files": {"documents": documents, "workbooks": workbooks or []},
            "expect": expect,
            "route": route,
            "scored": scored_by(expect, route, human),
            "min_tier": max(indices) if indices else 0,
            "note": note,
        }
        self.questions.append(question)
        return question

    # ---------------------------------------------------------------------------------------------- the data

    def readable(self, record: dict) -> bool:
        return bool(record["readable"])

    def names(self, entity_id: str) -> dict:
        from knowledge_sandbox.cast import person_forms

        return person_forms(self.cast.by_id[entity_id])

    def docs_containing(self, term: str, with_ocr: bool = False) -> list:
        """The readable documents whose text has `term` as whole words. Scanned ones are separate: their text
        is what the recognition reads, so they may or may not be found."""
        out = []
        for record in self.records:
            if not record["readable"] or bool(record["ocr"]) != with_ocr:
                continue
            if with_ocr:
                text = fold_text(" ".join(record.get("ocr_pages", [])))
            else:
                text = self.folded.get(record["path"], "")
            if has_phrase(text, term):
                out.append(record["path"])
        return out


def default_selection(block: str) -> dict:
    if block == "documents":
        return {"documents": "all", "workbooks": "none"}
    if block == "spreadsheets":
        return {"documents": "none", "workbooks": "all"}
    return {"documents": "all", "workbooks": "all"}


def scored_by(expect: dict, route: list, human: bool) -> dict:
    """What the scorer decides by itself from this expectation."""
    auto = []
    for key in ("facts", "patterns", "value", "values", "date", "listing", "total", "count", "count_range", "first", "groups", "ordered", "columns", "refusal", "nudge", "must_not",
                "must_not_patterns", "sources_all", "sources_any", "page"):
        if expect.get(key) not in (None, [], "", False):
            auto.append(key)
    if route:
        auto.append("route")
    return {"auto": auto, "human": human}


# ------------------------------------------------------------------------------------------------------ build


def build(library, books, seed: int, slice_only: bool) -> list:
    from . import book_docs, book_mixed, book_tables

    builder = Builder(library, books, seed, slice_only)
    book_docs.add_all(builder)
    book_tables.add_all(builder)
    book_mixed.add_all(builder)
    return builder.questions


# ------------------------------------------------------------------------------------------------- renderings


def render_csv(questions: list) -> str:
    out = io.StringIO()
    writer = csv.writer(out, delimiter=";", lineterminator="\n")
    writer.writerow(["id", "block", "feature", "format", "question", "files", "expected", "route", "scored by the program", "needs a reading"])
    for q in questions:
        files = q["files"]["documents"] + q["files"]["workbooks"]
        writer.writerow([
            q["id"], q["block"], f"{q['feature']} {F.FEATURES[q['feature']][1]}", q["format"], q["text"],
            " | ".join(files), describe_expectation(q["expect"]), " | ".join(q["route"]),
            ", ".join(q["scored"]["auto"]), "yes" if q["scored"]["human"] else "",
        ])
    return out.getvalue()


def describe_expectation(expect: dict) -> str:
    parts = []
    if expect.get("facts"):
        parts.append("contains " + " + ".join("/".join(group) for group in expect["facts"]))
    if expect.get("patterns"):
        parts.append("matches " + " + ".join(expect["patterns"]))
    if expect.get("value") is not None:
        parts.append(f"number {expect['value']}")
    if expect.get("values"):
        parts.append("numbers " + " and ".join(str(v) for v in expect["values"]))
    if expect.get("date"):
        parts.append(f"date {expect['date']}")
    if expect.get("listing") is not None:
        parts.append(f"lists {len(expect['listing'])} file(s), total {expect.get('total')}")
    if expect.get("columns"):
        parts.append("names " + ", ".join(expect["columns"]))
    if expect.get("refusal"):
        parts.append("must say the documents do not hold it")
    if expect.get("nudge"):
        parts.append(f"must refuse: {expect['nudge']}")
    if expect.get("must_not"):
        parts.append("must not contain " + " / ".join(expect["must_not"]))
    if expect.get("sources_all"):
        parts.append("cites all of " + ", ".join(expect["sources_all"]))
    if expect.get("sources_any"):
        parts.append("cites " + " or ".join(expect["sources_any"]))
    if expect.get("page"):
        parts.append(f"on page {expect['page']}")
    return "; ".join(parts)


def render_markdown(questions: list, manifest: dict) -> str:
    lines = [
        "# The question book", "",
        f"{len(questions)} questions: " + ", ".join(f"{count} {block}" for block, count in manifest.get("questions_by_block", {}).items()) + ".",
        "Every name, number and file is invented. Generated by `scripts/gen-knowledge-sandbox.py --profile demo`.", "",
    ]
    block = None
    for q in questions:
        if q["block"] != block:
            block = q["block"]
            lines += [f"## {block}", ""]
        files = q["files"]["documents"] + q["files"]["workbooks"]
        lines += [
            f"### {q['id']} - {q['feature']} {F.FEATURES[q['feature']][1]}" + (f" ({q['format']})" if q["format"] else ""),
            "", f"> {q['text']}", "",
            f"- Files: {', '.join(files) if files else '-'}",
            f"- Expected: {describe_expectation(q['expect']) or '-'}",
            f"- Road: {' or '.join(q['route'])}",
            f"- Scored by the program: {', '.join(q['scored']['auto']) or 'nothing'}" + ("; needs a reading" if q["scored"]["human"] else ""),
            "",
        ]
    return "\n".join(lines) + "\n"
