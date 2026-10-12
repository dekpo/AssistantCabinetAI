"""The workbooks of the sandbox (the Data folder).

Headers in French, in the shapes a practice really has: `Patient | Médecin traitant | Date | Montant`, split
`Nom` and `Prénom` columns, snake_case exports (`nom_patient`, `medecin_traitant`), a CP1252 CSV with `;`, a
sheet of free notes (must be rejected), a column called `Nom` that holds codes (must be rejected), formula cells,
`N/A` and empty cells. The same table in a format the product does not read (ODS, XLS) is written beside.

Every row that names someone is recorded (entity, form, sheet, column, row) so the ground truth knows exactly
where each person is.
"""

from __future__ import annotations

import datetime as dt
import random
from pathlib import Path

from . import docs as D
from . import writers as W
from .cast import Cast, Entity, ascii_keep_case, person_forms, fold
from .legacy import write_xls

EPOCH = dt.date(1899, 12, 30)


def serial(day: dt.date) -> int:
    return (day - EPOCH).days


class Book:
    def __init__(self, path: str, fmt: str, sheets: list, readable: bool):
        self.path = path
        self.fmt = fmt
        self.sheets = sheets  # [(name, rows)]
        self.readable = readable
        self.mentions: list = []  # {"entity","form","sheet","column","rows":[...]}
        self.notes: list = []

    def mention(self, entity: Entity, form: str, sheet: str, column: str, row: int) -> None:
        for m in self.mentions:
            if m["entity"] == entity.id and m["sheet"] == sheet and m["column"] == column and m["form"] == form:
                m["rows"].append(row)
                return
        self.mentions.append({"entity": entity.id, "form": form, "sheet": sheet, "column": column, "rows": [row]})

    def public(self) -> dict:
        return {
            "path": self.path,
            "format": self.fmt,
            "readable": self.readable,
            "sheets": [{"name": name, "rows": len(rows) - 1} for name, rows in self.sheets],
            "mentions": self.mentions,
            "notes": self.notes,
        }


def _patient_cell(rng: random.Random, e: Entity) -> tuple:
    roll = rng.random()
    forms = person_forms(e)
    if roll < 0.55:
        return forms["full"], "full"
    if roll < 0.85:
        return f"{e.family} {e.given}", "surname_given"
    return forms["surname_initial"], "surname_initial"


def build_workbooks(cast: Cast, seed: int, out: Path, invoices: list, contracts: list, extra: int = 0) -> list[Book]:
    rng = random.Random(f"tables-{seed}")
    books: list[Book] = []
    out.mkdir(parents=True, exist_ok=True)
    patients = [e for e in cast.people if e.role == "patient"]
    swiss = [e for e in patients if e.country == "CH"]
    french = [e for e in patients if e.country != "CH"]

    # --- 1. appointments, two sheets ---------------------------------------------------------------------------
    sheets = []
    book = Book("rendez-vous-2026.xlsx", "xlsx", [], True)
    for month, name, count in ((9, "Septembre", 60), (10, "Octobre", 42)):
        rows = [["Date", "Patient", "Médecin traitant", "Motif", "Montant", "Caisse"]]
        for i in range(count):
            e = _weighted_patient(rng, patients)
            cell, form = _patient_cell(rng, e)
            day = dt.date(2026, month, rng.randint(1, 30 if month == 9 else 28))
            while day.weekday() == 6:
                day += dt.timedelta(days=1)
            doctor = rng.choices(
                [cast.owner, cast.by_id["d-martin-paul"], cast.by_id["d-martin-pierre"], cast.by_id["d-vuagniaux-anne-sophie"]],
                weights=[60, 25, 8, 7],
            )[0]
            insurer = _insurer_of(rng, e, cast)
            rows.append([
                ("date", serial(day)), cell, f"{doctor.title or 'Dr'} {doctor.given} {doctor.family}",
                rng.choice(["Consultation", "Renouvellement d'ordonnance", "Suivi", "Certificat", "Vaccination", "Bilan annuel"]),
                rng.choice([30, 55, 60, 100]), insurer,
            ])
            book.mention(e, form, name, "Patient", len(rows))
            book.mention(doctor, "title_full", name, "Médecin traitant", len(rows))
        sheets.append((name, rows))
    book.sheets = sheets
    W.write_xlsx(out / book.path, sheets)
    books.append(book)
    # the same table in OpenDocument: not read by the product
    twin = Book("rendez-vous-2026.ods", "ods", sheets, False)
    twin.mentions = [dict(m) for m in book.mentions]
    W.write_ods(out / twin.path, sheets)
    books.append(twin)

    # --- 2. supplier invoices -----------------------------------------------------------------------------------
    rows = [["Date", "Fournisseur", "N° facture", "Montant HT", "TVA", "Statut"]]
    book = Book("factures-fournisseurs-2026.xlsx", "xlsx", [], True)
    for inv in invoices:
        supplier = cast.by_id[inv["supplier"]]
        rows.append([
            ("date", serial(inv["date"])), supplier.canonical, inv["number"], round(inv["ht"], 2), round(inv["vat"], 2),
            rng.choice(["payée", "payée", "à payer"]),
        ])
        book.mention(supplier, "full", "Factures", "Fournisseur", len(rows))
        entity = cast.identifiers.get(inv["number"])
        if entity is not None:
            book.mention(entity, "invoice", "Factures", "N° facture", len(rows))
    suppliers = [cast.org(k) for k in ("medsupply", "papeterie-lac", "bureautique-leman", "pharma-depot", "imprimerie-chablais", "sig", "edf", "swisscom", "orange")]
    for i in range(24 + extra):
        supplier = rng.choice(suppliers)
        day = dt.date(2026, rng.randint(1, 9), rng.randint(1, 28))
        number = f"FAC-2026-{200 + i:03d}"
        ht = round(rng.uniform(40, 1900), 2)
        rows.append([("date", serial(day)), supplier.canonical, number, ht, round(ht * 0.2, 2), rng.choice(["payée", "à payer"])])
        book.mention(supplier, "full", "Factures", "Fournisseur", len(rows))
    book.sheets = [("Factures", rows)]
    W.write_xlsx(out / book.path, book.sheets)
    books.append(book)

    # --- 3. Swiss patients, split Nom / Prénom, AVS ------------------------------------------------------------
    rows = [["Nom", "Prénom", "Date de naissance", "N° AVS", "Caisse maladie", "Commune", "NPA", "Téléphone", "E-mail"]]
    book = Book("patients-ch.xlsx", "xlsx", [], True)
    chosen = swiss[: 60 + extra]
    for e in chosen:
        insurer = _insurer_of(rng, e, cast, force_ch=True)
        rows.append([
            e.family, e.given, ("date", serial(dt.date(e.birth_year, rng.randint(1, 12), rng.randint(1, 28)))),
            e.ids.get("avs", ""), insurer, e.commune[0], e.commune[1], e.ids.get("phone", ""), e.ids.get("email", ""),
        ])
        book.mention(e, "split_columns", "Feuil1", "Nom", len(rows))
        avs = e.ids.get("avs")
        if avs:
            book.mention(cast.identifier("avs", avs, e.id), "avs", "Feuil1", "N° AVS", len(rows))
    notes = [["Notes"], ["Liste tenue à jour par le secrétariat. À vérifier avec les caisses avant chaque fin de trimestre."],
             ["Les patients frontaliers figurent aussi dans le fichier français."]]
    book.sheets = [("Feuil1", rows), ("Notes", notes)]
    book.notes.append("sheet Notes holds free text: must not be read as a table of names")
    W.write_xlsx(out / book.path, book.sheets)
    books.append(book)

    # --- 4. French patients, CP1252 CSV, snake_case headers --------------------------------------------------------
    rows = [["nom_patient", "medecin_traitant", "NIR", "Ville", "Code postal"]]
    book = Book("patients-fr.csv", "csv", [], True)
    for e in french[: 60 + extra]:
        forms = person_forms(e)
        cell, form = (forms["reordered"], "reordered") if rng.random() < 0.5 else (forms["full"], "full")
        if not W.can_cp1252(cell):
            # what a Windows export does with a letter its code page lacks: the nearest plain letter
            cell, form = ascii_keep_case(cell), "no_accent"
        doctor = cast.owner if rng.random() < 0.7 else cast.by_id["d-martin-paul"]
        rows.append([cell, f"{doctor.title or 'Dr'} {doctor.family}", e.ids.get("nir", ""), e.commune[0], e.commune[1]])
        book.mention(e, form, "csv", "nom_patient", len(rows))
        book.mention(doctor, "title_surname", "csv", "medecin_traitant", len(rows))
        nir = e.ids.get("nir")
        if nir:
            book.mention(cast.identifier("nir", nir, e.id), "nir", "csv", "NIR", len(rows))
    book.sheets = [("csv", rows)]
    W.write_csv(out / book.path, rows, delimiter=";", encoding="cp1252")
    books.append(book)

    # --- 5. contracts ----------------------------------------------------------------------------------------------
    rows = [["Fournisseur", "Numéro de contrat", "Début", "Fin", "Montant annuel"]]
    book = Book("contrats-2026.xlsx", "xlsx", [], True)
    for c in contracts:
        supplier = cast.by_id[c["supplier"]]
        rows.append([supplier.canonical, c["number"], ("date", serial(c["date"])), ("date", serial(c["end"])), round(c["annual"], 2)])
        book.mention(supplier, "full", "Contrats", "Fournisseur", len(rows))
        entity = cast.identifiers.get(c["number"])
        if entity is not None:
            book.mention(entity, "contract", "Contrats", "Numéro de contrat", len(rows))
    book.sheets = [("Contrats", rows)]
    W.write_xlsx(out / book.path, book.sheets)
    books.append(book)
    twin = Book("contrats-2026.xls", "xls", book.sheets, False)
    twin.mentions = [dict(m) for m in book.mentions]
    write_xls(out / twin.path, book.sheets)
    books.append(twin)

    # --- 6. duty roster: formulas, N/A, empty cells, free notes, a column "Nom" of codes ---------------------------
    rows = [["Semaine", "Médecin", "Remarques"]]
    book = Book("planning-garde-2026.xlsx", "xlsx", [], True)
    doctors = [e for e in cast.people if e.role == "doctor" and e.designed][:6] + [cast.owner]
    for week in range(1, 41):
        doctor = doctors[week % len(doctors)]
        remark = rng.choice(["", "", "N/A", "échange demandé", "férié", ""])
        rows.append([week, f"{doctor.title or 'Dr'} {doctor.family}", remark])
        book.mention(doctor, "title_surname", "Gardes", "Médecin", len(rows))
    totals = [["Médecin", "Nombre de semaines"]]
    for i, doctor in enumerate(doctors, start=2):
        totals.append([f"{doctor.title or 'Dr'} {doctor.family}", ("formula", f'COUNTIF(Gardes!B:B,"{doctor.title or "Dr"} {doctor.family}")', 5)])
    codes = [["Nom", "Zone"]] + [[f"{chr(65 + i % 6)}{i:02d}", rng.choice(["Nord", "Sud"])] for i in range(12)]
    notes = [["Remarques générales"], ["Les gardes de fin d'année sont attribuées à la réunion de novembre."]]
    book.sheets = [("Gardes", rows), ("Totaux", totals), ("Codes", codes), ("Notes", notes)]
    book.notes.append("sheet Codes: a column called Nom that holds zone codes (not names): must be rejected")
    book.notes.append("sheet Notes: free text: must be rejected")
    W.write_xlsx(out / book.path, book.sheets)
    books.append(book)
    return books


def _weighted_patient(rng: random.Random, patients: list) -> Entity:
    total = sum(max(e.weight, 0.01) for e in patients)
    roll = rng.random() * total
    acc = 0.0
    for e in patients:
        acc += max(e.weight, 0.01)
        if roll <= acc:
            return e
    return patients[-1]


def _insurer_of(rng: random.Random, e: Entity, cast: Cast, force_ch: bool = False) -> str:
    if force_ch or e.country == "CH":
        key = rng.choice(["helsana", "css", "groupe-mutuel", "sanitas", "swica", "assura"])
    else:
        key = rng.choice(["cpam74", "cpam69", "mgen", "harmonie"])
    org = cast.org(key)
    return org.short or org.canonical
