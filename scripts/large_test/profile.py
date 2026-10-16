"""The `demo` profile of `scripts/gen-knowledge-sandbox.py`: the library and the book of the large test.

    python scripts/gen-knowledge-sandbox.py --profile demo --out data/large-test/library
    python scripts/gen-knowledge-sandbox.py --profile demo --slice --out fixtures/large-test-slice

Writes under `<out>`: `documents/`, `data/`, the ground truth (`entities.json`, `documents.json`,
`workbooks.json`), the book (`questions.json`, `book.csv`, `book.md`) and `manifest.json`. A fixed seed gives
the same bytes; the first N documents of a longer library are the N documents of a shorter one.
"""

from __future__ import annotations

import json
import shutil
from pathlib import Path

from . import book as BOOK
from . import library as LIB
from . import workbooks as WB

DEFAULT_SEED = 20261016
# The full library: documents in the stream (readable text documents are about 85 % of them) and the scale of
# the workbook families (1.0 is 515 files).
FULL = {"documents": 1250, "scale": 1.0}
# The versioned slice: about a hundred documents and forty spreadsheets, thirty questions.
SLICE = {"documents": 100, "scale": 0.08}


def build_demo(out: Path, documents: int | None, seed: int, slice_only: bool) -> dict:
    shape = SLICE if slice_only else FULL
    total = documents or shape["documents"]
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    library = LIB.DemoLibrary(out, total, seed)
    library.build_documents()
    library.add_template()
    books = WB.build_workbooks(library.cast, seed, out / "data", library.invoices, library.contracts, shape["scale"])

    mentioned: set = set()
    for record in library.records:
        mentioned.update(m["entity"] for m in record["mentions"])
        mentioned.update(record["about"])
    for workbook in books:
        mentioned.update(workbook.people)
    entities = [e.public() for e in sorted(library.cast.by_id.values(), key=lambda e: e.id) if e.id in mentioned]

    questions = BOOK.build(library, books, seed, slice_only)

    manifest = {
        "profile": "demo",
        "slice": slice_only,
        "seed": seed,
        "documents": len(library.records),
        "readable_text_documents": sum(1 for r in library.records if r["readable"] and not r["ocr"]),
        "readable_documents": sum(1 for r in library.records if r["readable"]),
        "ocr_documents": sum(1 for r in library.records if r["ocr"]),
        "documents_by_format": _count(library.records, lambda r: r["format"]),
        "workbooks": len(books),
        "workbooks_by_format": _count(books, lambda b: b.fmt),
        "workbooks_by_family": _count(books, lambda b: b.family),
        "workbook_traps": _count([t for b in books for t in b.traps], lambda t: t),
        "questions": len(questions),
        "questions_by_block": _count(questions, lambda q: q["block"]),
        "generated_by": "scripts/gen-knowledge-sandbox.py --profile demo",
    }
    _write(out / "entities.json", entities)
    _write(out / "documents.json", library.records)
    _write(out / "workbooks.json", [b.public() for b in books])
    _write(out / "questions.json", questions)
    _write(out / "manifest.json", manifest)
    (out / "book.csv").write_bytes(BOOK.render_csv(questions).encode("utf-8"))
    (out / "book.md").write_bytes(BOOK.render_markdown(questions, manifest).encode("utf-8"))
    return manifest


def _write(path: Path, value) -> None:
    path.write_bytes((json.dumps(value, ensure_ascii=False, indent=1) + "\n").encode("utf-8"))


def _count(items, key) -> dict:
    out: dict = {}
    for item in items:
        out[key(item)] = out.get(key(item), 0) + 1
    return dict(sorted(out.items()))
