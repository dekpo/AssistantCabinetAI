"""Truth and scoring for the file profile (docs/LEXICAL-RETRIEVAL-DESIGN.md, sections 5 and 9).

Two jobs, both deterministic and both fed by the knowledge sandbox's own ground truth, so no new
generator is needed:

* ``name_queries``: every way the owner said a file must be found by its name (5.3), with the files
  each query must return. A query that several files satisfy expects all of them.
* ``score``: compares a profile dump (written by the product's own test, one object per file) with
  the truth, field by field: how often a filled field is right (precision) and how often a field the
  truth knows is filled at all (fill rate). A wrong value is worse than an empty one, so precision is
  the figure that must hold.

Profile dump, one object per file::

    {"path": "...", "business_type": "letter" | null, "document_date": "2026-02-12" | null}

Usage::

    python scripts/file_profile_score.py --sandbox fixtures/knowledge-sandbox --emit-name-queries out.json
    python scripts/file_profile_score.py --sandbox fixtures/knowledge-sandbox --profiles dump.json
"""

import argparse
import json
import re
import sys
import unicodedata
from collections import defaultdict
from pathlib import Path

# The business types of docs/LEXICAL-RETRIEVAL-DESIGN.md 5.1, from the sandbox's own kinds.
KIND_TO_TYPE = {
    "lettre_confrere": "letter",
    "lettre_patient": "letter",
    "courrier_pharmacie": "letter",
    "courrier_avocat": "letter",
    "courrier_caisse": "insurer_statement",
    "resultats_labo": "lab_result",
    "facture_fournisseur": "invoice",
    "facture_patient": "invoice",
    "ordonnance": "prescription",
    "attestation_assurance": "certificate",
    "certificat": "certificate",
    "devis": "quote",
    "pv_reunion": "meeting_minutes",
    "convocation": "appointment_letter",
}
DATE_PREFIX = re.compile(r"^\d{4}-\d{2}-\d{2}[_-]?")


def fold(text: str) -> str:
    """Lowercase without accents, the folding the product applies to names and queries."""
    decomposed = unicodedata.normalize("NFKD", text)
    return "".join(c for c in decomposed if not unicodedata.combining(c)).lower()


def split_name(path: str) -> tuple[str, str, str]:
    """(file name, stem, extension) of a relative path."""
    name = path.rsplit("/", 1)[-1]
    stem, dot, extension = name.rpartition(".")
    return (name, stem, extension) if dot else (name, name, "")


def name_words(path: str) -> list[str]:
    """The searchable words of a file's name and folders: folded, cut at every non-alphanumeric
    character and between a letter and a digit; a date is not a word."""
    _, stem, _ = split_name(path)
    folders = path.rsplit("/", 1)[0].replace("/", " ") if "/" in path else ""
    words: list[str] = []
    for part in (DATE_PREFIX.sub("", stem), folders):
        for chunk in re.split(r"[^0-9a-z]+", fold(part)):
            words.extend(re.findall(r"[a-z]+|[0-9]+", chunk))
    return [w for w in words if not re.fullmatch(r"\d{4}|\d{2}", w)]


def truth_from_sandbox(sandbox: Path) -> dict:
    documents = json.loads((sandbox / "documents.json").read_text(encoding="utf-8"))
    workbooks = json.loads((sandbox / "workbooks.json").read_text(encoding="utf-8"))
    entities = json.loads((sandbox / "entities.json").read_text(encoding="utf-8"))
    # Every way the sandbox writes each entity, folded: a role's surface or linked name is right when it is one.
    forms = {
        entity["id"]: {fold(entity["canonical"]), *(fold(form) for form in entity.get("forms", {}).values())}
        for entity in entities
    }
    files = {}
    for doc in documents:
        files[doc["path"]] = {
            "domain": "documents",
            "format": doc["extension"],
            "business_type": KIND_TO_TYPE.get(doc["kind"], "other"),
            "document_date": doc.get("date") or None,
            "subjects": doc.get("about", []),
            "about_forms": [sorted(forms[entity]) for entity in doc.get("about", []) if entity in forms],
            "name_entities": [m["entity"] for m in doc["mentions"] if m["where"] == "filename"],
        }
    for book in workbooks:
        files[book["path"]] = {
            "domain": "data",
            "format": book["format"],
            "sheets": [sheet["name"] for sheet in book["sheets"]],
        }
    return files


def name_queries(truth: dict) -> list[dict]:
    """Every query that must find a file by its name (design note 5.3), with the files it must return."""
    words_of = {path: name_words(path) for path in truth}
    queries: dict[str, dict] = {}

    def add(kind: str, text: str, expected: set[str]) -> None:
        if text and expected:
            queries.setdefault(f"{kind}|{text}", {"kind": kind, "query": text, "expected": sorted(expected)})

    for path in truth:
        name, stem, _ = split_name(path)
        no_date = DATE_PREFIX.sub("", stem)
        same = lambda value, key: {p for p in truth if key(p) == value}
        add("full_name", name, same(name, lambda p: split_name(p)[0]))
        add("stem", stem, same(stem, lambda p: split_name(p)[1]))
        add("no_date_prefix", no_date, same(no_date, lambda p: DATE_PREFIX.sub("", split_name(p)[1])))
        words = [w for w in words_of[path] if not w.isdigit()]
        if len(words) >= 2:
            pair = [words[1], words[0]]
            add("two_words_reversed", " ".join(pair),
                {p for p, ws in words_of.items() if all(w in ws for w in pair)})
        long_words = [w for w in words if len(w) >= 5]
        if long_words:
            prefix = long_words[0][:3]
            add("word_prefix", prefix,
                {p for p, ws in words_of.items() if any(w.startswith(prefix) for w in ws)})
        add("capitals", no_date.upper().replace("-", " ").replace("_", " "),
            same(no_date, lambda p: DATE_PREFIX.sub("", split_name(p)[1])))
    return list(queries.values())


def _stats(filled: int, correct: int, known: int) -> dict:
    return {
        "known": known,
        "filled": filled,
        "correct": correct,
        "precision": correct / filled if filled else None,
        "fill_rate": filled / known if known else None,
    }


def score(profiles: list[dict], truth: dict) -> dict:
    """Per field: ``filled`` (the product gave a value), ``correct``, ``known`` (the truth has one),
    ``precision`` = correct / filled and ``fill_rate`` = filled / known. A truth of ``other`` is right
    for an unfilled field and for ``other``; it counts as known only when a value was given.
    ``by_format`` repeats the figures per file format, and ``analysed`` restricts them to the files the
    dump holds (a format the product does not read yet is then not counted against the fill rate).
    A dump entry of the data folder is ignored: these fields describe documents."""
    by_path = {
        profile["path"]: profile for profile in profiles if profile.get("domain", "documents") == "documents"
    }
    report = {}
    for field in ("business_type", "document_date"):
        counts = defaultdict(lambda: [0, 0, 0])
        for path, expected in truth.items():
            if expected.get("domain") != "documents":
                continue
            wanted = expected.get(field)
            given = by_path.get(path, {}).get(field)
            groups = [("all", True), (expected.get("format", "?"), True), ("analysed", path in by_path)]
            for name, counted in groups:
                if not counted:
                    continue
                if wanted is not None and wanted != "other":
                    counts[name][2] += 1
                if given is None:
                    continue
                counts[name][0] += 1
                if given == wanted:
                    counts[name][1] += 1
        report[field] = {
            **_stats(*counts["all"]),
            "analysed": _stats(*counts["analysed"]),
            "by_format": {name: _stats(*value) for name, value in sorted(counts.items()) if name not in ("all", "analysed")},
        }
    return report


def score_roles(profiles: list[dict], truth: dict) -> dict:
    """The ``subject`` role against the sandbox's ``about`` list. ``known``: documents that are about
    someone and were analysed; ``filled``: those given a subject; ``correct``: those whose subject (the
    linked name, or the surface when nothing was linked) is one of the ways the sandbox writes that person."""
    by_path = {
        profile["path"]: profile for profile in profiles if profile.get("domain", "documents") == "documents"
    }
    known = filled = correct = 0
    for path, expected in truth.items():
        if expected.get("domain") != "documents" or not expected.get("about_forms") or path not in by_path:
            continue
        known += 1
        subjects = [role for role in by_path[path].get("roles", []) if role.get("role") == "subject"]
        if not subjects:
            continue
        filled += 1
        accepted = {form for forms in expected["about_forms"] for form in forms}
        if any(
            (role.get("canonical") and fold(role["canonical"]) in accepted)
            or any(form in fold(role.get("surface", "")) for form in accepted)
            for role in subjects
        ):
            correct += 1
    return _stats(filled, correct, known)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--sandbox", required=True, type=Path)
    parser.add_argument("--profiles", type=Path)
    parser.add_argument("--emit-name-queries", type=Path)
    args = parser.parse_args(argv)
    truth = truth_from_sandbox(args.sandbox)
    if args.emit_name_queries:
        queries = name_queries(truth)
        args.emit_name_queries.write_text(json.dumps(queries, ensure_ascii=False, indent=1), encoding="utf-8")
        by_kind = defaultdict(int)
        for query in queries:
            by_kind[query["kind"]] += 1
        print(f"{len(queries)} name queries over {len(truth)} files: {dict(by_kind)}")
    if args.profiles:
        profiles = json.loads(args.profiles.read_text(encoding="utf-8"))
        report = score(profiles, truth)
        report["subject_role"] = score_roles(profiles, truth)
        print(json.dumps(report, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
