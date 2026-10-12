"""Assemble the sandbox: write the documents and the workbooks, and the ground truth beside them.

    <out>/documents/...        the Documents folder (what the owner points the application at)
    <out>/data/...             the Data folder
    <out>/entities.json        every invented entity, its forms, its identifiers
    <out>/documents.json       every document: format, tier, and everything it names (exact strings)
    <out>/workbooks.json       every workbook: sheets, and where each entity is
    <out>/questions.json       the questions and what they must find
    <out>/manifest.json        parameters, tiers, counts per format
    <out>/README.md

Tiers are prefixes of the stream: the first 30 documents are the 30 of tier 30, the first 100 the 100 of tier 100.
"""

from __future__ import annotations

import datetime as dt
import json
import random
import shutil
from pathlib import Path

from . import docs as D
from . import plan as PL
from . import questions as Q
from . import tables
from . import writers as W
from .cast import Cast, fold, slug
from .legacy import write_ppt_stand_in

TIERS = [30, 100, 200, 500, 1000, 2000]
READABLE = {"pdf", "scan-pdf", "docx", "txt", "md", "jpg", "png"}
EXTENSION = {
    "xlsx": "xlsx", "csv": "csv",
    "pdf": "pdf", "scan-pdf": "pdf", "docx": "docx", "txt": "txt", "md": "md", "jpg": "jpg", "png": "png",
    "odt": "odt", "rtf": "rtf", "eml": "eml", "vtt": "vtt", "pptx": "pptx", "odp": "odp", "ppt": "ppt",
}
FORMAT_WEIGHTS = {
    "pdf": [("pdf", 72), ("scan-pdf", 7), ("jpg", 5), ("png", 2), ("docx", 8), ("odt", 4), ("rtf", 2)],
    "docx": [("docx", 70), ("pdf", 12), ("odt", 7), ("txt", 5), ("md", 4), ("rtf", 2)],
    "txt": [("txt", 62), ("md", 16), ("docx", 8), ("odt", 4), ("rtf", 3), ("pdf", 7)],
    "eml": [("eml", 55), ("txt", 40), ("docx", 5)],
    "any": [("pdf", 34), ("docx", 24), ("txt", 20), ("md", 8), ("jpg", 5), ("odt", 5), ("rtf", 4)],
    "vtt": [("vtt", 100)],
    "slides": [("pptx", 45), ("odp", 25), ("ppt", 30)],
}
FOLDERS = {
    "lettre_confrere": "courriers-entrants/{ym}",
    "resultats_labo": "laboratoire/{ym}",
    "courrier_caisse": "caisses-assurances/{country_dir}",
    "demande_remboursement": "caisses-assurances/suisse",
    "facture_fournisseur": "comptabilite/factures",
    "devis": "comptabilite/devis",
    "contrat": "contrats",
    "attestation_assurance": "contrats/assurances",
    "ordonnance": "patients/ordonnances",
    "certificat": "patients/certificats",
    "lettre_patient": "patients/courriers",
    "facture_patient": "patients/honoraires",
    "courrier_avocat": "juridique",
    "convocation": "reunions",
    "pv_reunion": "reunions",
    "transcription": "reunions/enregistrements",
    "note_interne": "administratif/notes",
    "procedure": "administratif/procedures",
    "mail": "messagerie",
    "rapport_activite": "administratif/rapports",
    "releve_garde": "administratif/planning",
    "presentation": "formation",
    "courrier_pharmacie": "courriers-entrants/pharmacies",
    "long": "documentation",
    "commandes_tableau": "administratif/commandes",
}


def pick(rng: random.Random, table):
    total = sum(w for _, w in table)
    roll = rng.random() * total
    acc = 0.0
    for item, weight in table:
        acc += weight
        if roll <= acc:
            return item
    return table[-1][0]


def encodable(doc: D.Doc) -> bool:
    return all(W.can_cp1252(text) for text in [doc.title] + doc.paragraphs)


def choose_format(doc: D.Doc, flags: dict, rng: random.Random) -> str:
    fmt = flags.get("fmt")
    if fmt is None:
        family = doc.prefer or "any"
        if family == "long":
            fmt = "pdf"
        else:
            fmt = pick(rng, FORMAT_WEIGHTS.get(family, FORMAT_WEIGHTS["any"]))
    if fmt == "pdf" and not encodable(doc):
        fmt = "docx"
    return fmt


def scan_pages(doc: D.Doc, per_page: int = 34, width: int = 64) -> list:
    lines = W._wrap(([doc.title] if doc.title else []) + doc.paragraphs, width)
    pages = [lines[i:i + per_page] for i in range(0, len(lines), per_page)] or [[""]]
    return pages[:6]


def write_doc(path: Path, doc: D.Doc, fmt: str) -> int:
    """Write the document in its format; return its number of pages (1 for formats without pages)."""
    if fmt == "pdf":
        return W.write_pdf_text(path, doc.title, doc.paragraphs)
    if fmt == "scan-pdf":
        return W.write_pdf_scanned(path, scan_pages(doc))
    if fmt in ("jpg", "png"):
        W.write_image(path, scan_pages(doc)[0], fmt)
        return 1
    if fmt == "docx":
        W.write_docx(path, doc.title, doc.paragraphs)
    elif fmt == "txt":
        W.write_text(path, doc.title, doc.paragraphs)
    elif fmt == "md":
        W.write_markdown(path, doc.title, doc.paragraphs)
    elif fmt == "odt":
        W.write_odt(path, doc.title, doc.paragraphs)
    elif fmt == "rtf":
        W.write_rtf(path, doc.title, doc.paragraphs)
    elif fmt == "eml":
        W.write_eml(path, doc.headers or {"Subject": doc.title}, doc.paragraphs)
    elif fmt == "vtt":
        W.write_vtt(path, doc.cues)
    elif fmt == "pptx":
        W.write_pptx(path, doc.title, doc.slides)
    elif fmt == "odp":
        W.write_odp(path, doc.slides)
    elif fmt == "ppt":
        write_ppt_stand_in(path, doc.title, doc.slides)
    elif fmt == "xlsx":
        W.write_xlsx(path, [("Commandes", doc.table)])
    elif fmt == "csv":
        W.write_csv(path, doc.table, delimiter=";", encoding="cp1252")
    else:
        raise ValueError(fmt)
    return 1


def page_of_needle(doc: D.Doc, sentence: str, lines_per_page: int = 46, width: int = 92) -> int:
    """The PDF page that holds the start of a planted sentence, by replaying the writer's layout."""
    lines = [doc.title, ""] + W._wrap(doc.paragraphs, width)
    key = " ".join(sentence.split("{")[0][:30].split())
    for number, start in enumerate(range(0, len(lines), lines_per_page), start=1):
        joined = " ".join(" ".join(lines[start:start + lines_per_page]).split())
        if key in joined:
            return number
    return 0


class Library:
    def __init__(self, out: Path, total: int, seed: int):
        self.out = out
        self.total = total
        self.seed = seed
        self.cast = Cast(seed)
        self.rng_dates = random.Random(f"dates-{seed}")
        self.rng_doc = random.Random(f"doc-{seed}")
        self.rng_fmt = random.Random(f"fmt-{seed}")
        self.rng_cov = random.Random(f"cov-{seed}")
        self.rng_rand = random.Random(f"rand-{seed}")
        self.records: list = []
        self.used_paths: set = set()
        self.seen: dict = {}
        self.invoices: list = []
        self.contracts: list = []
        self.invoice_counter = [13 + 100]
        self.contract_counter = [40 + 100]

    # numbering for generated (not hand written) documents starts above the hand written ones
    def next_invoice(self) -> str:
        self.invoice_counter[0] += 1
        return f"FAC-2026-{self.invoice_counter[0]:03d}"

    def next_contract(self) -> str:
        self.contract_counter[0] += 1
        return f"CTR-2026-{self.contract_counter[0]:04d}"

    def next_date(self) -> dt.date:
        start = dt.date(2026, 1, 5)
        return start + dt.timedelta(days=self.rng_dates.randint(0, 277))

    # -----------------------------------------------------------------------------------------------------------

    def place(self, doc: D.Doc, fmt: str, flags: dict) -> str:
        rng = self.rng_fmt
        folder = flags.get("fixed_folder")
        if folder is None:
            template = FOLDERS.get(doc.kind, "divers")
            folder = template.format(ym=f"{doc.date.year}-{doc.date.month:02d}", country_dir="suisse" if doc.country == "CH" else "france")
            if fmt in ("scan-pdf", "jpg", "png") and rng.random() < 0.5:
                folder = f"scans/{doc.date.year}-{doc.date.month:02d}"
        stem = flags.get("fixed_name")
        if stem is None:
            stem = f"{doc.date.isoformat()}_{doc.name_hint or doc.kind}"
            if doc.named_after and rng.random() < 0.55:
                person = self.cast.by_id[doc.named_after[0]]
                stem += f"-{slug(person.family)}-{slug(person.given)}"
                doc.mentions.append({"entity": person.id, "text": f"{slug(person.family)}-{slug(person.given)}", "form": "filename", "where": "filename"})
        stem = stem if flags.get("fixed_name") is not None else _clean(stem)
        relative = f"{folder}/{stem}.{EXTENSION[fmt]}" if folder else f"{stem}.{EXTENSION[fmt]}"
        counter = 2
        base = relative
        while relative in self.used_paths:
            relative = base.replace(f".{EXTENSION[fmt]}", f"-{counter}.{EXTENSION[fmt]}")
            counter += 1
        self.used_paths.add(relative)
        return relative

    def produce(self, kind: str, fn, flags: dict, long_pages: int = 0) -> None:
        doc = D.Doc(kind=kind, date=self.next_date(), country=flags.get("country", "FR"), cast=self.cast, rng=self.rng_doc)
        fn(doc)
        fmt = choose_format(doc, flags, self.rng_fmt)
        if doc.headers and fmt != "eml":
            # an e-mail saved as a document keeps its header lines in the text
            doc.paragraphs.insert(0, chr(10).join(f"{key}: {value}" for key, value in doc.headers.items()))
        relative = self.place(doc, fmt, flags)
        target = self.out / "documents" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        pages = write_doc(target, doc, fmt)
        index = len(self.records) + 1
        tier = next((t for t in TIERS if index <= t), TIERS[-1])
        mentioned = []
        for m in doc.mentions:
            if m["where"] == "text" and m["entity"] not in mentioned:
                mentioned.append(m["entity"])
        for entity_id in mentioned + doc.about:
            self.seen[entity_id] = self.seen.get(entity_id, 0) + 1
        if doc.numbers.get("kind") == "invoice":
            self.invoices.append({**doc.numbers})
        if doc.numbers.get("kind") == "contract":
            self.contracts.append({**doc.numbers})
        needles = {}
        for key, sentence in list(doc.facts.items()):
            if key.startswith("needle_page_"):
                needles[key] = {"sentence": sentence, "page": page_of_needle(doc, sentence) if fmt == "pdf" else 0}
        ocr_pages: list = []
        if fmt in ("scan-pdf", "jpg", "png"):
            shown = scan_pages(doc) if fmt == "scan-pdf" else scan_pages(doc)[:1]
            ocr_pages = ["\n".join(page) for page in shown]
        text = " ".join([doc.title] + doc.paragraphs).casefold()
        terms = [word for word in Q.KEYWORDS if word.casefold() in text]
        self.records.append({
            "index": index,
            "path": relative,
            "format": fmt,
            "extension": EXTENSION[fmt],
            "kind": kind,
            "date": doc.date.isoformat(),
            "country": doc.country,
            "readable": fmt in READABLE,
            "ocr": fmt in ("scan-pdf", "jpg", "png"),
            "pages": pages,
            "tier": tier,
            "special": flags.get("special", ""),
            "about": doc.about,
            "mentions": doc.mentions,
            "terms": terms,
            "ocr_pages": ocr_pages,
            "ocr_level": W.scan_level(scan_pages(doc)[0]) if fmt in ("scan-pdf", "jpg", "png") else None,
            "facts": {k: v for k, v in doc.facts.items() if not k.startswith("needle_page_")},
            "needles": needles,
        })

    # -----------------------------------------------------------------------------------------------------------

    # Long documents, at fixed positions of the stream: (position, key, format, pages, title, file name, needles).
    # A needle is a sentence planted on one page, with the question that has it as its answer and what the
    # answer must contain. The first two are in the versioned core; the others only exist in larger libraries.
    LONG = [
        (35, "guide", "pdf", 150, "Guide de pratique du cabinet", "guide-pratique-cabinet", [
            (12, "La formation à l'hygiène est déclarée auprès de {org}.", [("o-ordre-74", "full", "{org}")],
             "Auprès de qui la formation à l'hygiène est-elle déclarée ?", ["Ordre des médecins de la Haute-Savoie"]),
            (42, "Les collecteurs d'aiguilles sont enlevés le jeudi par {org}.", [("o-pharma-depot", "full", "{org}")],
             "Quel prestataire enlève les collecteurs d'aiguilles ?", ["Pharma-Dépôt Romandie SA"]),
            (87, "Le code de l'armoire à pharmacie est renouvelé chaque premier lundi du mois par {who}.", [("s-colas", "full", "{who}")],
             "Qui renouvelle le code de l'armoire à pharmacie ?", ["Sandrine Colas"]),
            (101, "En cas de doute sur une homonymie, le secrétariat appelle {who} avant d'enregistrer le dossier.", [("o-marchetti", "title_full", "{who}")],
             "Qui appeler en cas de doute sur une homonymie ?", ["Marchetti"]),
            (110, "Le guide a été relu par {who} et par {org}.", [("d-rapin-jean-luc", "title_full", "{who}"), ("o-hug", "acronym", "{org}")],
             "Qui a relu le guide de pratique ?", ["Rapin"]),
        ]),
        (66, "manual", "docx", 110, "Manuel qualité", "manuel-qualite", [
            (33, "Le contrat de maintenance des copieurs est tenu avec {org}.", [("o-bureautique-leman", "full", "{org}")],
             "Avec qui est tenu le contrat de maintenance des copieurs ?", ["Bureautique Léman SA"]),
            (58, "Les écarts de la chaîne du froid sont signalés à {who}.", [("s-bensaid", "full", "{who}")],
             "À qui signaler un écart de la chaîne du froid ?", ["Nadia Bensaïd"]),
            (95, "Les contrôles annuels des appareils sont réalisés par un organisme agréé, puis transmis à {org}.", [("o-ars", "acronym", "{org}")],
             "À qui sont transmis les contrôles annuels des appareils ?", ["ARS"]),
        ]),
        (250, "recueil", "pdf", 260, "Recueil des procédures et des fiches de poste", "recueil-procedures", [
            (140, "Le registre des accès aux dossiers est archivé chaque trimestre par {who}.", [("s-colas", "title_full", "{who}")],
             "Qui archive le registre des accès aux dossiers ?", ["Colas"]),
            (181, "Les attestations d'assurance sont renouvelées auprès de {org} avant le 31 janvier.", [("o-suva", "full", "{org}")],
             "Auprès de qui renouvelle-t-on les attestations d'assurance ?", ["SUVA"]),
        ]),
        (400, "protocole", "docx", 150, "Protocole de soins et d'accueil", "protocole-soins", [
            (90, "Une température du réfrigérateur au-dessus de 8 degrés est signalée sans délai à {who}.", [("d-martin-paul", "title_full", "{who}")],
             "À qui signaler une température du réfrigérateur au-dessus de 8 degrés ?", ["Martin"]),
        ]),
        (600, "annuaire", "pdf", 380, "Annuaire interne et conventions", "annuaire-conventions", [
            (222, "La convention avec {org} prévoit un délai de réponse de quarante-huit heures.", [("o-unilabs", "full", "{org}")],
             "Quel délai de réponse la convention avec Unilabs prévoit-elle ?", ["quarante-huit"]),
            (271, "La liaison avec {org} est assurée par {who}.", [("o-chuv", "acronym", "{org}"), ("d-vuagniaux-anne-sophie", "title_full", "{who}")],
             "Qui assure la liaison avec le CHUV ?", ["Vuagniaux"]),
        ]),
        (800, "formation", "docx", 180, "Manuel de formation des nouveaux arrivants", "manuel-formation", [
            (77, "La procédure de gestion des plaintes est présentée par {who}.", [("p-rochat-sophie", "full", "{who}")],
             "Qui présente la procédure de gestion des plaintes ?", ["Sophie Rochat"]),
        ]),
    ]

    def produce_long(self, entry: tuple) -> None:
        _, key, fmt, pages, title, hint, needles = entry
        flags = {"country": "FR", "fmt": fmt, "fixed_name": hint, "special": "long"}
        plain = [(page, sentence, mentions) for page, sentence, mentions, _, _ in needles]

        def fn(doc: D.Doc) -> None:
            D.long_document(doc, pages, plain, title, hint)

        self.produce("long", fn, flags)
        record = self.records[-1]
        for page, sentence, _, question, contains in needles:
            slot = record["needles"].setdefault(f"needle_page_{page}", {"sentence": sentence, "page": 0})
            slot["question"] = question
            slot["contains"] = contains

    def build_documents(self) -> None:
        explicit = PL.plan_documents(self.cast, self.seed, self.total)
        for kind, fn, flags in explicit:
            if len(self.records) >= self.total:
                return
            self.produce(kind, fn, flags)
            self._maybe_long()
        coverage = PL.coverage_specs(self.cast, self.rng_cov, self.seen, self.next_invoice, self.next_contract)
        for kind, fn, flags in coverage:
            if len(self.records) >= self.total:
                return
            self.produce(kind, fn, flags)
            self._maybe_long()
        while len(self.records) < self.total:
            kind, fn, flags = PL.random_spec(self.cast, self.rng_rand, self.next_invoice, self.next_contract)
            self.produce(kind, fn, flags)
            self._maybe_long()

    def _maybe_long(self) -> None:
        for entry in self.LONG:
            if len(self.records) == entry[0] and self.total > entry[0]:
                self.produce_long(entry)


def _clean(stem: str) -> str:
    head, _, rest = stem.partition("_")
    return f"{head}_{slug(rest)}" if rest else slug(head)


def build(out: Path, total: int, seed: int, extra_workbooks: int = 0) -> dict:
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    library = Library(out, total, seed)
    library.build_documents()
    books = tables.build_workbooks(library.cast, seed, out / "data", library.invoices, library.contracts, extra_workbooks)

    # entities that appear anywhere
    mentioned: set = set()
    for record in library.records:
        for m in record["mentions"]:
            mentioned.add(m["entity"])
        mentioned.update(record["about"])
    for book in books:
        for m in book.mentions:
            mentioned.add(m["entity"])
    for e in library.cast.people + library.cast.orgs:
        if e.designed:
            mentioned.add(e.id)
    entities = [e.public() for e in sorted(library.cast.by_id.values(), key=lambda e: e.id) if e.id in mentioned]

    questions = Q.build_questions(library, books)
    manifest = {
        "seed": seed,
        "documents": len(library.records),
        "workbooks": len(books),
        "tiers": [t for t in TIERS if t < len(library.records)] + [len(library.records)],
        "by_format": _count(library.records, "format"),
        "readable": sum(1 for r in library.records if r["readable"]),
        "unreadable": sum(1 for r in library.records if not r["readable"]),
        "ocr": sum(1 for r in library.records if r["ocr"]),
        "pages_total": sum(r["pages"] for r in library.records),
        "questions": len(questions),
        "generated_by": "scripts/gen-knowledge-sandbox.py",
    }
    (out / "entities.json").write_text(json.dumps(entities, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (out / "documents.json").write_text(json.dumps(library.records, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (out / "workbooks.json").write_text(json.dumps([b.public() for b in books], ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (out / "questions.json").write_text(json.dumps(questions, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    shutil.copy2(Path(__file__).with_name("README.md"), out / "README.md")
    return manifest


def _count(records: list, key: str) -> dict:
    out: dict = {}
    for r in records:
        out[r[key]] = out.get(r[key], 0) + 1
    return dict(sorted(out.items()))
