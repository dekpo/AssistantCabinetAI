"""Generate the long-document corpus of the long-documents programme (tab 2, lot 0).

Measurement tooling, not product code. Fictional French content only; no person, number or code is real.

    python scripts/gen-long-documents.py --out data/long-documents
    python scripts/gen-long-documents.py --base data/knowledge-sandbox-1000 --out data/long-library-1000

The output is a **sandbox-shaped root** (`documents/`, `documents.json`, `questions.json`, `entities.json`,
`workbooks.json`, `manifest.json`), so the existing laboratory reads it with `ACAI_SANDBOX_DIR`. Plus
`sections.json`: for every document, its real sections (label, level, first and last page), which is the truth the
structure work is measured against. With `--base`, the base library is copied first and the long documents are
appended after its last document (index, tier and question ids continue; nothing of the base changes).

Documents (full size; `--scale` shrinks every size for a plumbing run):

    guide-qualite.pdf             500 pages, text layer, PDF outline, running header and numbered footer
    recueil-procedures.pdf      1 000 pages, same
    encyclopedie-pratique.pdf   2 000 pages, same
    manuel-formation.docx         500 logical pages, heading styles
    livre-blanc.md              1 000 logical pages, `#` headings
    rapport-annuel.txt          1 000 logical pages, numbered headings
    archives-numerisees.pdf     2 000 scanned pages (no text layer); the text of each page is in documents.json,
                                which the laboratory's fake OCR answers

A "logical page" of a DOCX, MD or TXT file is 44 body lines; the product reads such a file as one single page.

Every text document carries the same planted facts, at known pages and in a known section:

    heading       the only carrier of the key word is a section heading; the sentence under it names the answer
    prose         one sentence, a code
    list          a titled list in one page
    list_split    the list title is the last line of a page, the items start the next page
    table_split   the header row is the last line of a page, the rows start the next page
    hyphen_line   the key word is cut at the end of a line ("pharmaco-" / "vigilance")
    hyphen_page   the key word is cut across a page break
    absent        a question on a topic the corpus covers, about a fact that is nowhere (the refusal tests)

A question never contains the fact it asks for. `contains` is what a passage must carry (all of it, in one passage)
to count as the fact: for a table it is the header row AND the row, for a list its title AND the item.
"""

from __future__ import annotations

import argparse
import json
import random
import re
import shutil
import sys
import textwrap
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from knowledge_sandbox import writers as W  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SEED = 20261014
FULL_SIZES = {"pdf": 500, "pdf_big": 1000, "pdf_huge": 2000, "docx": 500, "md": 1000, "txt": 1000, "scan": 2000}

# key -> (path, format, title, header words)
SPECS = {
    "pdf": ("longs/guide-qualite.pdf", "pdf", "Guide qualité du cabinet"),
    "pdf_big": ("longs/recueil-procedures.pdf", "pdf", "Recueil des procédures"),
    "pdf_huge": ("longs/encyclopedie-pratique.pdf", "pdf", "Encyclopédie pratique"),
    "docx": ("longs/manuel-formation.docx", "docx", "Manuel de formation"),
    "md": ("longs/livre-blanc.md", "md", "Livre blanc de l'organisation"),
    "txt": ("longs/rapport-annuel.txt", "txt", "Rapport annuel d'activité"),
    "scan": ("longs/archives-numerisees.pdf", "scan-pdf", "Archives numérisées"),
}
EXTENSION = {"pdf": "pdf", "docx": "docx", "md": "md", "txt": "txt", "scan-pdf": "pdf"}
GEOMETRY = {"pdf": (44, 92), "docx": (44, 92), "md": (44, 92), "txt": (44, 92), "scan-pdf": (20, 60)}

# kind -> fraction of the document where it is planted, in order
PLAN_TEXT = [("heading", 0.05), ("prose", 0.14), ("list", 0.24), ("table_split", 0.36), ("list_split", 0.48),
             ("hyphen_line", 0.60), ("hyphen_page", 0.72), ("prose", 0.90)]
PLAN_SCAN = [("prose", 0.30), ("prose", 0.85)]

TOPICS = ["Hygiène", "Déchets", "Données", "Accueil", "Facturation", "Urgences", "Matériel"]
SUBJECTS = ["Le secrétariat", "L'équipe de soins", "Le responsable qualité", "Chaque praticien", "Le référent du site",
            "La direction", "Le service technique", "L'assistante de gestion"]
COMPLEMENTS = ["chaque semaine", "avant toute utilisation", "à la fin de la journée", "dans un délai de huit jours",
               "selon la procédure en vigueur", "en présence d'un témoin", "sans exception", "après accord de la direction",
               "dès réception du courrier", "une fois par trimestre", "avec un justificatif daté", "dans le registre commun"]
VERBS_OBJECTS = {
    "Hygiène": [("désinfecte", "les surfaces de contact"), ("contrôle", "le stock de produits désinfectants"),
                ("renouvelle", "les consommables d'examen"), ("vérifie", "le cycle de stérilisation du matériel"),
                ("consigne", "les opérations de nettoyage"), ("remplace", "les gants à usage unique")],
    "Déchets": [("trie", "les déchets à risque infectieux"), ("ferme", "le collecteur d'objets piquants"),
                ("archive", "le bordereau de suivi de l'enlèvement"), ("rapporte", "les médicaments périmés"),
                ("signale", "tout débordement des conteneurs"), ("pèse", "les sacs avant l'enlèvement")],
    "Données": [("sauvegarde", "les dossiers sur le second site"), ("renouvelle", "le mot de passe du poste"),
                ("consigne", "chaque demande d'accès au registre"), ("détruit", "les dossiers arrivés à échéance"),
                ("chiffre", "les copies destinées à l'extérieur"), ("contrôle", "les droits d'accès de chaque utilisateur")],
    "Accueil": [("vérifie", "l'identité de la personne reçue"), ("reporte", "le rendez-vous en cas de retard"),
                ("note", "les coordonnées à jour"), ("oriente", "les personnes vers la salle d'attente"),
                ("remet", "le questionnaire d'accueil"), ("confirme", "le rendez-vous de la semaine suivante")],
    "Facturation": [("établit", "la facture à la fin de la consultation"), ("relance", "les impayés de plus de trente jours"),
                    ("classe", "les justificatifs par mois"), ("vérifie", "la franchise annuelle de l'assuré"),
                    ("contrôle", "le tarif en vigueur"), ("transmet", "le dossier de remboursement")],
    "Urgences": [("vérifie", "la date de péremption du chariot d'urgence"), ("teste", "la charge du défibrillateur"),
                 ("affiche", "les numéros d'appel à l'accueil"), ("organise", "la remise à niveau des gestes de secours"),
                 ("contrôle", "le niveau de la bouteille d'oxygène"), ("consigne", "chaque intervention d'urgence")],
    "Matériel": [("planifie", "l'étalonnage annuel des appareils"), ("compare", "les devis de renouvellement du parc"),
                 ("renégocie", "les contrats de maintenance"), ("inventorie", "le matériel de bureau"),
                 ("signale", "toute panne au service technique"), ("range", "les clés de l'armoire commune")],
}
SHORT_FILLERS = ["Voir aussi l'annexe correspondante.", "Se reporter au registre commun.", "Cette règle s'applique à tous.",
                 "Aucune dérogation n'est admise.", "La mise à jour est datée et signée.", "Le suivi est assuré chaque mois.",
                 "Un exemplaire est conservé au secrétariat.", "Le document de référence est tenu à jour."]
KEYS = ["Vermeer", "Kestrel", "Orion", "Vesper", "Altaïr", "Borée", "Cassiopée", "Dorian", "Eridan", "Falcon", "Gallion",
        "Halcyon", "Ibis", "Jasmin", "Kodiak", "Lyra", "Mistral", "Nacre", "Oural", "Pégase", "Quartz", "Rigel", "Sirocco",
        "Tamaris", "Ulysse", "Vigie", "Wapiti", "Xénon", "Yucca", "Zéphyr", "Aubrac", "Balsa", "Cèdre", "Dune", "Écume",
        "Fjord", "Garrigue", "Hêtre", "Iris", "Jonc", "Karst", "Lagune", "Mélèze", "Nénuphar", "Ombrelle", "Pinède",
        "Quenelle", "Roselière", "Sorbier", "Tilleul"]
GIVEN = ["Odile", "Armand", "Nadège", "Corentin", "Solène", "Basile", "Maëlle", "Gaspard", "Irène", "Lucien", "Thaïs", "Aurèle"]
FAMILY = ["Pasquereau", "Barbaroux", "Quenelle-Ricard", "Tessonneau", "Vauquelin", "Marcoulet", "Desbiolles", "Cherpitel",
          "Fontenilles", "Gerbaudie", "Lacoutière", "Mouchotte", "Pétremand", "Rouvillois", "Saugrain", "Tavernier"]
HYPHEN_TERMS = [("pharmaco", "vigilance"), ("radio", "protection"), ("décontami", "nation"), ("immuno", "suppresseur"),
                ("thermo", "régulation"), ("électro", "cardiographie"), ("bio", "compatibilité"), ("hémo", "vigilance"),
                ("ostéo", "densitométrie"), ("chimio", "thérapie"), ("cryo", "conservation"), ("spiro", "métrie"),
                ("photo", "protection"), ("néphro", "protection"), ("gastro", "entérologie"), ("hémo", "dialyse"),
                ("neuro", "stimulation"), ("angio", "plastie"), ("cyto", "toxicité"), ("endo", "scopie")]
ABSENT = [("Quelle est la durée de la franchise {x} ?", "franchise"), ("Quel est le tarif de la consultation de nuit au site {x} ?", "tarif"),
          ("Qui est le responsable du site {x} ?", "responsable"), ("Quel est le code d'accès du local {x} ?", "code d'accès")]
ABSENT_WORDS = ["Zircon-9", "Obsidienne-4", "Béryl-12", "Topaze-7"]
TABLE_HEADER = "Site | Délai (jours) | Tarif (CHF)"


def wrap(text: str, width: int) -> list:
    return textwrap.wrap(text, width=width, break_long_words=False, break_on_hyphens=False) or [""]


# ------------------------------------------------------------------------------------------------- the flow


class Block:
    def __init__(self, kind: str, lines: list, label: str = ""):
        self.kind, self.lines, self.label = kind, lines, label


class Flow:
    """A document as a sequence of blocks over lines, paginated at `per_page` body lines. Headings follow a
    schedule; planted items are placed at an exact line, so the page and the section are known, not guessed."""

    def __init__(self, rng: random.Random, pages: int, geometry: tuple):
        self.rng, self.pages = rng, pages
        self.per_page, self.width = geometry
        self.blocks: list = []
        self.count = 0
        self.events: list = []  # (line index, level, label)
        self.chapter = 0
        self.sub = 0
        self.chapter_pages = max(6, pages // 10)
        self.section_pages = max(2, self.chapter_pages // 4)
        self.next_chapter_page = 1
        self.next_section_page = 1
        self.topic = TOPICS[0]

    def page_of(self, index: int) -> int:
        return index // self.per_page + 1

    def add(self, block: Block) -> int:
        start = self.count
        self.blocks.append(block)
        self.count += len(block.lines)
        return start

    def heading(self, level: int, label: str) -> None:
        start = self.add(Block(f"h{level}", [label], label))
        self.events.append((start, level, label))

    def schedule_heading(self) -> None:
        page = self.page_of(self.count)
        if page >= self.next_chapter_page:
            self.chapter += 1
            self.sub = 0
            self.topic = self.rng.choice(TOPICS)
            self.heading(1, f"Chapitre {self.chapter} — {self.topic}")
            self.next_chapter_page = page + self.chapter_pages
            self.next_section_page = page
        if self.page_of(self.count) >= self.next_section_page:
            self.sub += 1
            self.heading(2, f"{self.chapter}.{self.sub} {self.rng.choice(TOPICS)} : {self.rng.choice(['principes', 'règles', 'suivi', 'contrôles', 'registre', 'responsabilités'])}")
            self.next_section_page = self.page_of(self.count) + self.section_pages

    def sentence(self) -> str:
        verb, thing = self.rng.choice(VERBS_OBJECTS[self.topic])
        return f"{self.rng.choice(SUBJECTS)} {verb} {thing} {self.rng.choice(COMPLEMENTS)}."

    def paragraph(self, lines_wanted: int) -> list:
        text, lines = "", []
        while True:
            candidate = (text + " " + self.sentence()).strip()
            wrapped = wrap(candidate, self.width)
            if len(wrapped) > lines_wanted and lines:
                return lines
            text, lines = candidate, wrapped
            if len(lines) >= lines_wanted:
                return lines[:lines_wanted] if len(lines) > lines_wanted else lines

    def fill_to(self, target: int) -> None:
        assert self.count <= target, f"the flow is already past line {target} ({self.count})"
        while self.count < target:
            room = target - self.count
            if room >= 5 and self.page_of(self.count) >= min(self.next_chapter_page, self.next_section_page):
                before = self.count
                self.schedule_heading()
                if self.count > target:
                    raise AssertionError("a heading overshot the target")
                if self.count != before:
                    continue
            room = target - self.count
            if room < 3:
                for _ in range(room):
                    self.add(Block("pad", [self.rng.choice(SHORT_FILLERS)]))
                break
            lines = self.paragraph(min(room, self.rng.randint(4, 9)))
            self.add(Block("para", lines))

    def label_at(self, index: int) -> str:
        found = ""
        level_one = ""
        for start, level, label in self.events:
            if start > index:
                break
            if level == 2:
                found = label
            else:
                level_one = label
                found = ""
        return found or level_one

    def sections(self) -> list:
        out = []
        for position, (start, level, label) in enumerate(self.events):
            end_index = self.count - 1
            for later_start, later_level, _ in self.events[position + 1:]:
                if later_level <= level:
                    end_index = later_start - 1
                    break
            out.append({"label": label, "level": level, "first_page": self.page_of(start),
                        "last_page": self.page_of(max(start, end_index))})
        return out


# ------------------------------------------------------------------------------------------------- planting


class Pool:
    """Unique keys, codes and people across the whole corpus."""

    def __init__(self, rng: random.Random):
        self.rng = rng
        self.keys = [f"{key}{suffix}" for suffix in ("", "-B", "-C", "-D") for key in KEYS]
        rng.shuffle(self.keys)
        self.hyphens = list(HYPHEN_TERMS)
        rng.shuffle(self.hyphens)
        self.codes: set = set()
        self.people: set = set()

    def key(self) -> str:
        return self.keys.pop()

    def code(self) -> str:
        while True:
            letters = "ABCDEFGHJKLMNPQRSTUVWXYZ"
            code = f"{self.rng.choice(letters)}{self.rng.choice(letters)}-{self.rng.randint(1000, 9999)}-{self.rng.choice(letters)}{self.rng.choice(letters)}"
            if code not in self.codes:
                self.codes.add(code)
                return code

    def person(self) -> str:
        while True:
            name = f"{self.rng.choice(GIVEN)} {self.rng.choice(FAMILY)}"
            if name not in self.people:
                self.people.add(name)
                return name

    def hyphen(self) -> tuple:
        return self.hyphens.pop()


def plant(flow: Flow, kind: str, fraction: float, pool: Pool, spacing: int, floor: int) -> dict:
    """Place one fact; return its record (question text, what a passage must carry, where it is)."""
    per = flow.per_page
    page = max(floor, round(fraction * flow.pages))
    page = min(page, flow.pages - spacing)
    start_of = lambda offset: (page - 1) * per + offset  # noqa: E731
    record: dict = {"plant": kind}
    if kind == "prose":
        flow.fill_to(start_of(8))
        key, code = pool.key(), pool.code()
        at = flow.add(Block("para", wrap(f"Le code d'accès du local {key} est {code}.", flow.width)))
        record.update(question=f"Quel est le code d'accès du local {key} ?", contains=[code], line=at)
    elif kind == "heading":
        flow.fill_to(start_of(6))
        key, person = pool.key(), pool.person()
        flow.sub += 1
        flow.heading(2, f"{flow.chapter}.{flow.sub} Procédure du site {key}")
        flow.next_section_page = flow.page_of(flow.count) + flow.section_pages
        at = flow.add(Block("para", wrap(f"Le responsable de ce site est {person}.", flow.width)))
        record.update(question=f"Qui est responsable du site {key} ?", contains=[person], line=at)
    elif kind in ("list", "list_split"):
        key, ref = pool.key(), f"FT-{pool.rng.randint(1000, 9999)}"
        title = f"Liste de contrôle du poste {key} :"
        items = ["- vérifier le niveau des réserves", "- consigner la date du dernier contrôle",
                 f"- remplacer le filtre référence {ref}", "- refermer le capot de protection",
                 "- signer la feuille de passage", "- ranger la clé dans l'armoire"]
        offset = per - 1 if kind == "list_split" else 10
        flow.fill_to(start_of(offset))
        at = flow.add(Block("list", [title] + items))
        record.update(question=f"Quelle référence de filtre faut-il remplacer sur le poste {key} ?", contains=[title, ref],
                      line=at + 2, header_page=flow.page_of(at))
    elif kind == "table_split":
        key = pool.key()
        days, tarif = pool.rng.randint(2, 30), pool.rng.randint(20, 400)
        row = f"{key} | {days} | {tarif}"
        rows = [row] + [f"{pool.key()} | {pool.rng.randint(2, 30)} | {pool.rng.randint(20, 400)}" for _ in range(6)]
        pool.rng.shuffle(rows)
        flow.fill_to(start_of(per - 1))
        at = flow.add(Block("table", [TABLE_HEADER] + rows))
        record.update(question=f"Quel est le délai pour le site {key} ?", contains=[TABLE_HEADER, row],
                      line=at + 1 + rows.index(row), header_page=flow.page_of(at))
    elif kind in ("hyphen_line", "hyphen_page"):
        head, tail = pool.hyphen()
        person = pool.person()
        offset = per - 1 if kind == "hyphen_page" else 10
        flow.fill_to(start_of(offset))
        first = f"La déclaration annuelle de {head}-"
        rest = wrap(f"{tail} est signée chaque année par {person}.", flow.width)
        at = flow.add(Block("para", [first] + rest))
        record.update(question=f"Qui signe la déclaration annuelle de {head + tail} ?", contains=[person], line=at + 1,
                      hyphenated=[head, tail], term=head + tail, header_page=flow.page_of(at))
    else:
        raise ValueError(kind)
    record["page"] = flow.page_of(record["line"])
    record["section"] = flow.label_at(record["line"])
    return record


# ------------------------------------------------------------------------------------------------- the writers


def pdf_string(text: str) -> bytes:
    return text.replace("\\", r"\\").replace("(", r"\(").replace(")", r"\)").encode("cp1252")


def write_pdf(path: Path, pages: list, outline: list) -> None:
    """A PDF with a text layer, one content stream per page, and a two-level outline (level, label, page)."""
    count = len(pages)
    font, pages_obj, outlines_obj = 1, 2 + 2 * count, 3 + 2 * count
    first_item = 4 + 2 * count
    catalog = first_item + len(outline)
    objects: list = [b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"]
    for lines in pages:
        stream = b"BT\n/F1 11 Tf\n56 790 Td\n15 TL\n" + b"".join(b"(" + pdf_string(line) + b") Tj\nT*\n" for line in lines) + b"ET"
        objects.append(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        objects.append(
            f"<< /Type /Page /Parent {pages_obj} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 {font} 0 R >> >> "
            f"/Contents {len(objects) + 1 - 1} 0 R >>".encode("latin-1"))
    kids = " ".join(f"{3 + 2 * i} 0 R" for i in range(count))
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {count} >>".encode("latin-1"))
    top = [i for i, (level, _, _) in enumerate(outline) if level == 1]
    objects.append(
        f"<< /Type /Outlines /First {first_item + top[0]} 0 R /Last {first_item + top[-1]} 0 R /Count {len(outline)} >>"
        .encode("latin-1"))
    parent_of, children = {}, {}
    current = None
    for i, (level, _, _) in enumerate(outline):
        if level == 1:
            current = i
            children[i] = []
        else:
            parent_of[i] = current
            children[current].append(i)
    for i, (level, label, page) in enumerate(outline):
        siblings = top if level == 1 else children[parent_of[i]]
        at = siblings.index(i)
        parent = outlines_obj if level == 1 else first_item + parent_of[i]
        body = b"<< /Title (" + pdf_string(label) + b") "
        body += f"/Parent {parent} 0 R /Dest [{3 + 2 * (page - 1)} 0 R /XYZ 0 790 null] ".encode("latin-1")
        if at > 0:
            body += f"/Prev {first_item + siblings[at - 1]} 0 R ".encode("latin-1")
        if at < len(siblings) - 1:
            body += f"/Next {first_item + siblings[at + 1]} 0 R ".encode("latin-1")
        if level == 1 and children[i]:
            body += f"/First {first_item + children[i][0]} 0 R /Last {first_item + children[i][-1]} 0 R /Count {len(children[i])} ".encode("latin-1")
        objects.append(body + b">>")
    objects.append(f"<< /Type /Catalog /Pages {pages_obj} 0 R /Outlines {outlines_obj} 0 R /PageMode /UseOutlines >>".encode("latin-1"))
    W._write_pdf_objects(path, objects, catalog)


def write_scan_stub(path: Path, count: int) -> None:
    """A PDF of `count` pages that are only a picture: no text layer, the OCR route. One shared 8x8 picture and one
    shared content stream keep a 2 000-page file small; the laboratory's fake OCR answers each page."""
    image_data = zlib.compress(bytes([250] * 64))
    objects = [
        f"<< /Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8 "
        f"/Filter /FlateDecode /Length {len(image_data)} >>\nstream\n".encode("latin-1") + image_data + b"\nendstream",
        b"<< /Length 31 >>\nstream\nq 595 0 0 842 0 0 cm /Im0 Do Q\nendstream",
    ]
    pages_obj = 3 + count
    for _ in range(count):
        objects.append(f"<< /Type /Page /Parent {pages_obj} 0 R /MediaBox [0 0 595 842] /Resources << /XObject << /Im0 1 0 R >> >> /Contents 2 0 R >>".encode("latin-1"))
    kids = " ".join(f"{3 + i} 0 R" for i in range(count))
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {count} >>".encode("latin-1"))
    objects.append(f"<< /Type /Catalog /Pages {pages_obj} 0 R >>".encode("latin-1"))
    W._write_pdf_objects(path, objects, len(objects))


def write_docx(path: Path, title: str, blocks: list) -> None:
    from xml.sax.saxutils import escape

    def para(text: str, style: str = "") -> str:
        props = f'<w:pPr><w:pStyle w:val="{style}"/></w:pPr>' if style else ""
        return f'<w:p>{props}<w:r><w:t xml:space="preserve">{escape(text)}</w:t></w:r></w:p>'

    parts = [para(title, "Title")]
    for block in blocks:
        if block.kind in ("h1", "h2"):
            parts.append(para(block.lines[0], "Heading1" if block.kind == "h1" else "Heading2"))
        else:
            parts += [para(line) for line in block.lines]
    ns = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    document = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{ns}"><w:body>{"".join(parts)}</w:body></w:document>'
    styles = (f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{ns}">'
              + "".join(f'<w:style w:type="paragraph" w:styleId="{s}"><w:name w:val="{n}"/></w:style>'
                        for s, n in (("Title", "Title"), ("Heading1", "heading 1"), ("Heading2", "heading 2"))) + "</w:styles>")
    content_types = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                     '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
                     '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
                     '<Default Extension="xml" ContentType="application/xml"/>'
                     '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
                     '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>'
                     "</Types>")
    rels = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
            "</Relationships>")
    doc_rels = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
                "</Relationships>")
    W._zip(path, [("[Content_Types].xml", content_types), ("_rels/.rels", rels), ("word/document.xml", document),
                  ("word/_rels/document.xml.rels", doc_rels), ("word/styles.xml", styles)])


def write_text_family(path: Path, fmt: str, title: str, blocks: list) -> None:
    out = [f"# {title}" if fmt == "md" else title, ""]
    for block in blocks:
        if block.kind == "h1":
            out += [f"# {block.lines[0]}" if fmt == "md" else block.lines[0], ""]
        elif block.kind == "h2":
            out += [f"## {block.lines[0]}" if fmt == "md" else block.lines[0], ""]
        else:
            out += block.lines + [""]
    path.write_text("\n".join(out), encoding="utf-8", newline="\n")


# ------------------------------------------------------------------------------------------------- one document


def build_document(out: Path, key: str, pages: int, pool: Pool, seed: int) -> tuple:
    path, fmt, title = SPECS[key]
    rng = random.Random(f"{seed}-{key}")
    geometry = GEOMETRY[fmt]
    flow = Flow(rng, pages, geometry)
    flow.schedule_heading()
    plan = PLAN_SCAN if fmt == "scan-pdf" else PLAN_TEXT
    spacing = 2 if fmt != "scan-pdf" else 1
    facts = []
    floor = 2
    for kind, fraction in plan:
        record = plant(flow, kind, fraction, pool, spacing, floor)
        floor = record["page"] + 3
        facts.append(record)
    flow.fill_to(pages * flow.per_page)
    assert flow.page_of(flow.count - 1) == pages, (key, flow.count)

    target = out / "documents" / path
    target.parent.mkdir(parents=True, exist_ok=True)
    header = f"{title} — version 3.2 — diffusion interne"
    body = [line for block in flow.blocks for line in block.lines]
    full_pages = [[header] + body[i * flow.per_page:(i + 1) * flow.per_page] + [f"Page {i + 1} / {pages}"] for i in range(pages)]
    sections = flow.sections()
    record: dict = {"path": path, "format": fmt, "extension": EXTENSION[fmt], "pages": pages, "ocr_pages": [], "ocr": False,
                    "logical_pages": fmt in ("docx", "md", "txt")}
    if fmt == "pdf":
        write_pdf(target, full_pages, [(s["level"], s["label"], s["first_page"]) for s in sections])
    elif fmt == "scan-pdf":
        write_scan_stub(target, pages)
        record["ocr_pages"] = ["\n".join(page) for page in full_pages]
        record["ocr"] = True
    elif fmt == "docx":
        write_docx(target, title, flow.blocks)
    else:
        write_text_family(target, fmt, title, flow.blocks)
    return record, facts, sections


# ------------------------------------------------------------------------------------------------- the whole corpus


def question_records(doc_path: str, fmt: str, facts: list, index: int, counter: list) -> list:
    out = []
    for fact in facts:
        counter[0] += 1
        q = {
            "id": f"LQ{counter[0]:03d}", "category": "planted", "text": fact["question"], "entities": [], "name_entities": [],
            "relevant_documents": [doc_path], "relevant_readable": [doc_path], "relevant_workbooks": [],
            "selection": {"documents": "all", "workbooks": "none"},
            "expect": {"resolution": "none", "between": [], "contains": fact["contains"]},
            "min_tier": index, "follow_up_of": None, "carry_over": [],
            "plant": fact["plant"], "fact_sections": [fact["section"]], "note": f"{fact['plant']} planted on page {fact['page']}",
        }
        q["fact_logical_pages" if fmt in ("docx", "md", "txt") else "fact_pages"] = [fact["page"]]
        for extra in ("header_page", "hyphenated", "term"):
            if extra in fact:
                q[extra] = fact[extra]
        out.append(q)
    return out


def absent_questions(counter: list, first_index: int) -> list:
    out = []
    for (template, topic), word in zip(ABSENT, ABSENT_WORDS):
        counter[0] += 1
        out.append({
            "id": f"LQ{counter[0]:03d}", "category": "absent", "text": template.format(x=word), "entities": [], "name_entities": [],
            "relevant_documents": [], "relevant_readable": [], "relevant_workbooks": [],
            "selection": {"documents": "all", "workbooks": "none"},
            "expect": {"resolution": "none", "between": [], "contains": []},
            "min_tier": first_index, "follow_up_of": None, "carry_over": [], "plant": "absent", "fact_sections": [],
            "absent_term": word, "topic_term": topic, "note": "nothing in the documents answers this",
        })
    return out


def build(out: Path, sizes: dict | None = None, base: Path | None = None, seed: int = DEFAULT_SEED) -> dict:
    sizes = FULL_SIZES if sizes is None else sizes
    out, base = Path(out), (Path(base) if base else None)
    if base is not None and out.resolve() == base.resolve():
        raise SystemExit("the output folder must not be the base library")
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    documents: list = []
    questions: list = []
    entities: list = []
    books: list = []
    manifest: dict = {"seed": seed, "documents": 0}
    if base is not None:
        shutil.copytree(base / "documents", out / "documents")
        if (base / "data").is_dir():
            shutil.copytree(base / "data", out / "data")
        read = lambda name: json.loads((base / name).read_text(encoding="utf-8"))  # noqa: E731
        documents, questions, entities, books, manifest = read("documents.json"), read("questions.json"), read("entities.json"), read("workbooks.json"), read("manifest.json")
    pool = Pool(random.Random(f"pool-{seed}"))
    counter = [sum(1 for q in questions if str(q.get("id", "")).startswith("LQ"))]
    next_index = max([d["index"] for d in documents], default=0) + 1
    first_index = next_index
    sections_all: dict = {}
    added = []
    for key in SPECS:
        if key not in sizes:
            continue
        record, facts, sections = build_document(out, key, sizes[key], pool, seed)
        _, fmt, _ = SPECS[key]
        record.update({"index": next_index, "kind": "long_corpus", "readable": True, "tier": next_index, "special": "long-corpus",
                       "about": [], "mentions": [], "terms": [], "facts": {}})
        documents.append(record)
        added.append(record)
        sections_all[record["path"]] = sections
        questions += question_records(record["path"], fmt, facts, next_index, counter)
        next_index += 1
    questions += absent_questions(counter, first_index)
    manifest.update({
        "documents": len(documents), "questions": len(questions),
        "long_corpus": {"sizes": {k: sizes[k] for k in SPECS if k in sizes}, "documents": [d["path"] for d in added],
                        "pages_total": sum(d["pages"] for d in added), "first_index": first_index},
        "generated_by": "scripts/gen-long-documents.py",
    })
    for name, value in (("documents.json", documents), ("questions.json", questions), ("entities.json", entities),
                        ("workbooks.json", books), ("manifest.json", manifest), ("sections.json", sections_all)):
        (out / name).write_text(json.dumps(value, ensure_ascii=False, indent=1), encoding="utf-8")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=ROOT / "data" / "long-documents", help="output folder (replaced if it exists)")
    parser.add_argument("--base", type=Path, help="a sandbox-shaped library to copy first; the long documents are appended to it")
    parser.add_argument("--scale", type=float, default=1.0, help="multiply every size (a plumbing run)")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    args = parser.parse_args()
    sizes = {k: max(12, int(v * args.scale)) for k, v in FULL_SIZES.items()}
    print(json.dumps(build(args.out, sizes, args.base, args.seed), ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
