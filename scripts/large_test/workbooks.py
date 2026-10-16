"""The spreadsheets of the `demo` profile: at least 500 files in XLSX, ODS and CSV (UTF-8 and Windows-1252), ten
families of the tables a practice keeps (appointments, supplier invoices, stock, hours, patient lists, contracts,
expenses, fees, training, deliveries), with the traps the tabular engine knows.

Every table is generated as Python values first; the files are written from them, and the book computes its
expected answers from the same values, never from the files. Money is kept in cents so a sum is exact.

Traps (`Workbook.traps`), each applied to a share of the files:

    title_row      the header is on row 3, under a title and a blank line
    merged_title   the title is one merged cell across the table (XLSX and ODS)
    formula_total  the last row of the amount column is a SUM formula with its cached value (XLSX and ODS): the
                   engine must refuse to total that column and say why
    empty_column   a column with nothing in it, in the middle of the table
    notes_sheet    a second sheet of free text that is not a table
    text_dates     dates written as text (dd/mm/yyyy or dd.mm.yyyy) instead of date cells
    swiss_numbers  1'234.56 and dd.mm.yyyy (CSV); the default CSV style is 1 234,56 and dd/mm/yyyy
"""

from __future__ import annotations

import datetime as dt
import random
from dataclasses import dataclass, field
from pathlib import Path

from knowledge_sandbox import writers as W
from knowledge_sandbox.cast import Cast, Entity, ascii_keep_case, person_forms, slug

from . import sheetfiles as S

MONTHS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"]

# family -> number of files in the full library
QUOTAS = {
    "rendez-vous": 90, "factures-fournisseurs": 60, "stock": 45, "planning-heures": 45, "patients-ch": 30,
    "patients-fr": 30, "contrats": 20, "compta-depenses": 55, "honoraires": 45, "formation-presences": 25,
    "livraisons": 65,
}
# Share of each format, in hundredths: 255 + 150 + 110 of 515.
FORMATS = [("xlsx", 50), ("csv", 28), ("ods", 22)]


@dataclass
class Sheet:
    name: str
    columns: list
    rows: list  # lists of Python values: str, int, float (money, two decimals), date, None
    title: str = ""
    merged_title: bool = False
    empty_column: int | None = None  # the index, in the written sheet, of a column that holds nothing
    formula_total: str = ""  # the name of the column whose last row is a SUM formula
    notes: bool = False  # a free-text sheet, not a table

    def header_row(self) -> int:
        """The row of the header as written, counted from 1."""
        return 3 if self.title else 1


@dataclass
class Workbook:
    path: str
    fmt: str  # xlsx | csv | ods
    family: str
    period: str
    entity: str  # a supplier, a doctor, a site...
    sheets: list
    traps: list = field(default_factory=list)
    encoding: str = "utf-8"  # csv
    numbers: str = "fr"  # csv: fr | ch | plain
    dates: str = "fr"  # csv: fr | ch | iso
    people: list = field(default_factory=list)  # entity ids named in the cells

    def public(self) -> dict:
        return {
            "path": self.path, "format": self.fmt, "family": self.family, "period": self.period, "entity": self.entity,
            "traps": self.traps, "encoding": self.encoding if self.fmt == "csv" else "",
            "numbers": self.numbers if self.fmt == "csv" else "", "readable": True, "usable": self.usable(),
            "sheets": [
                {"name": s.name, "header_row": s.header_row(), "columns": s.columns, "rows": len(s.rows), "table": not s.notes}
                for s in self.sheets
            ],
            "people": self.people,
        }

    def usable(self) -> bool:
        """Whether the engine takes the workbook for a table. One formula cell among fewer than 20 rows is more than
        5 % of them: the engine marks such a workbook unusable (red), `docs/DECISIONS.md`, Data Folder session."""
        sheet = self.table()
        return not (sheet.formula_total and len(sheet.rows) + 1 < 20)

    def table(self, sheet: int = 0) -> Sheet:
        return [s for s in self.sheets if not s.notes][sheet]


# -------------------------------------------------------------------------------------------------- helpers


def cents(value: float) -> int:
    return round(value * 100)


def money(rng: random.Random, low: float, high: float) -> float:
    return round(rng.uniform(low, high), 2)


def total_cents(values) -> int:
    return sum(cents(v) for v in values if v is not None)


def weekday_dates(rng: random.Random, year: int, month: int, count: int) -> list:
    days = []
    for _ in range(count):
        day = dt.date(year, month, rng.randint(1, 28))
        while day.weekday() == 6:
            day += dt.timedelta(days=1)
        days.append(day)
    return sorted(days)


def encodable(text: str, encoding: str) -> bool:
    return encoding != "cp1252" or W.can_cp1252(text)


class Pool:
    """People and places the cells draw from."""

    def __init__(self, cast: Cast):
        self.cast = cast
        self.patients = [e for e in cast.people if e.role == "patient"]
        self.designed = [e for e in self.patients if e.designed]
        self.swiss = [e for e in self.patients if e.country == "CH"]
        self.french = [e for e in self.patients if e.country != "CH"]
        self.doctors = [cast.owner] + [e for e in cast.people if e.role == "doctor" and e.designed][:6]
        self.staff = [e for e in cast.people if e.role == "staff"] + [cast.owner]
        self.suppliers = [o for o in cast.orgs if o.kind == "supplier"]

    def patient(self, rng: random.Random, encoding: str) -> Entity:
        """A designed patient half the time (they are in the documents too), anyone otherwise."""
        for _ in range(20):
            pool = self.designed if rng.random() < 0.5 and self.designed else self.patients
            e = rng.choice(pool)
            if encodable(person_forms(e)["full"], encoding):
                return e
        return self.designed[0]

    def doctor_label(self, e: Entity) -> str:
        return f"{e.title or 'Dr'} {e.given} {e.family}" if e.role != "owner" else f"Dr {e.given} {e.family}"


def name_cell(rng: random.Random, e: Entity, encoding: str) -> str:
    forms = person_forms(e)
    cell = forms["full"] if rng.random() < 0.7 else f"{e.family.upper()} {e.given}"
    return cell if encodable(cell, encoding) else ascii_keep_case(cell)


# ------------------------------------------------------------------------------------------------ families
# Each builder returns (sheets, people, entity). `rng` is the workbook's own generator.


def build_rendez_vous(rng, pool, year, month, site, encoding):
    people = []
    sheets = []
    weeks = rng.choice([1, 2, 3])
    for number in range(1, weeks + 1):
        count = rng.randint(14, 60)
        rows = []
        for day in weekday_dates(rng, year, month, count):
            patient = pool.patient(rng, encoding)
            doctor = rng.choice(pool.doctors)
            people += [patient.id, doctor.id]
            insurer = rng.choice(["Helsana", "CSS", "Groupe Mutuel", "Sanitas"] if site["country"] == "CH" else ["CPAM 74", "MGEN", "Harmonie"])
            rows.append([
                day, name_cell(rng, patient, encoding), pool.doctor_label(doctor),
                rng.choice(["Consultation", "Renouvellement", "Suivi", "Certificat", "Vaccination", "Bilan annuel"]),
                rng.choice([30.0, 55.0, 60.0, 100.0, 85.5]), insurer,
            ])
        sheets.append(Sheet(f"Semaine {number}" if weeks > 1 else "Rendez-vous", ["Date", "Patient", "Médecin", "Motif", "Montant", "Caisse"], rows))
    return sheets, people, site["name"]


def build_factures(rng, pool, year, month, supplier, invoices, encoding):
    rows = []
    for inv in invoices:
        ht = round(inv["ht"], 2)
        vat = round(inv["vat"], 2)
        ttc = round(ht + vat, 2)
        # One invoice in three is booked in the workbook for another amount than the document states (a typing
        # slip, a credit note): the mixed questions compare the two.
        if sum(int(c) for c in inv["number"] if c.isdigit()) % 3 == 0:
            ttc = round(ttc * 1.05, 2)
        rows.append([inv["date"], inv["number"], rng.choice(["Fournitures", "Maintenance", "Livraison", "Abonnement"]), ht, vat, ttc, rng.choice(["payée", "payée", "à payer"])])
    return [Sheet("Factures", ["Date", "N° facture", "Libellé", "Montant HT", "TVA", "Montant TTC", "Statut"], rows)], [supplier.id], supplier.canonical


def build_stock(rng, pool, year, month, category, encoding):
    articles = {
        "consommables": ["Gants d'examen", "Compresses", "Seringues 5 ml", "Aiguilles 21G", "Pansements", "Masques", "Draps d'examen", "Abaisse-langue"],
        "bureau": ["Papier A4", "Toner noir", "Classeurs", "Stylos", "Enveloppes", "Étiquettes", "Agrafes", "Post-it"],
        "pharmacie": ["Paracétamol 500", "Désinfectant", "Sérum physiologique", "Solution hydroalcoolique", "Vaccin test", "Pansement stérile"],
    }[category["name"]]
    suppliers = [o for o in pool.suppliers if o.country == category["country"]] or pool.suppliers
    rows = []
    wanted = rng.randint(12, 24)
    for i, name in enumerate(articles * 3):
        if i >= wanted:
            break
        rows.append([f"{name} {i // len(articles) + 1}" if i >= len(articles) else name, f"REF-{rng.randint(1000, 9999)}", rng.randint(0, 400), rng.choice([20, 50, 100]), money(rng, 0.4, 38.0), rng.choice(suppliers).canonical, dt.date(year, month, rng.randint(1, 28))])
    return [Sheet("Stock", ["Article", "Référence", "Quantité", "Stock minimum", "Prix unitaire", "Fournisseur", "Dernière commande"], rows)], [], category["name"]


def build_planning(rng, pool, year, month, team, encoding):
    rows = []
    staff = rng.sample(pool.staff, k=min(len(pool.staff), rng.randint(3, 5)))
    for day in weekday_dates(rng, year, month, rng.randint(20, 40)):
        person = rng.choice(staff)
        rows.append([day, f"{person.given} {person.family}" if person.role != "owner" else person.canonical, rng.choice(["Accueil", "Soins", "Administratif", "Garde"]), float(rng.choice([4, 6, 7.5, 8]))])
    return [Sheet("Heures", ["Date", "Collaborateur", "Poste", "Heures"], rows)], [p.id for p in staff], team["name"]


def build_patients_ch(rng, pool, letters, encoding):
    people = [e for e in pool.swiss if e.family[:1].upper() in letters and encodable(person_forms(e)["full"], encoding)]
    rows = []
    for e in people[:80]:
        rows.append([e.family, e.given, dt.date(e.birth_year, rng.randint(1, 12), rng.randint(1, 28)), e.ids.get("avs", ""), rng.choice(["Helsana", "CSS", "Groupe Mutuel", "Sanitas", "Swica"]), e.commune[0] if e.commune else "", e.commune[1] if e.commune else ""])
    return [Sheet("Patients", ["Nom", "Prénom", "Date de naissance", "N° AVS", "Caisse maladie", "Commune", "NPA"], rows)], [e.id for e in people[:80]], "".join(sorted(letters))


def build_patients_fr(rng, pool, letters, encoding):
    people = [e for e in pool.french if e.family[:1].upper() in letters and encodable(person_forms(e)["full"], encoding)]
    rows = []
    for e in people[:80]:
        rows.append([f"{e.family.upper()} {e.given}", e.ids.get("nir", ""), e.commune[0] if e.commune else "", e.commune[1] if e.commune else "", rng.choice(["Dr Marchetti", "Dr Martin"])])
    return [Sheet("Patients", ["nom_patient", "NIR", "Ville", "Code postal", "medecin_traitant"], rows)], [e.id for e in people[:80]], "".join(sorted(letters))


def build_contrats(rng, pool, year, month, group, contracts, encoding):
    rows = []
    for c in contracts:
        rows.append([pool.cast.by_id[c["supplier"]].canonical, c["number"], c["date"], c["end"], round(c["annual"], 2)])
    return [Sheet("Contrats", ["Fournisseur", "Numéro de contrat", "Début", "Fin", "Montant annuel"], rows)], [pool.cast.by_id[c["supplier"]].id for c in contracts], group


def build_compta(rng, pool, year, month, label, encoding):
    rows = []
    for day in weekday_dates(rng, year, month, rng.randint(18, 50)):
        debit = money(rng, 12, 1800) if rng.random() < 0.8 else None
        credit = money(rng, 30, 400) if debit is None else None
        rows.append([day, rng.choice(["Loyer", "Électricité", "Fournitures", "Téléphone", "Entretien", "Honoraires reçus", "Assurance"]), rng.choice(["Charges", "Frais généraux", "Recettes"]), debit, credit])
    return [Sheet("Journal", ["Date", "Libellé", "Catégorie", "Débit", "Crédit"], rows)], [], label


def build_honoraires(rng, pool, year, month, doctor, encoding):
    rows = []
    for day in weekday_dates(rng, year, month, rng.randint(15, 45)):
        patient = pool.patient(rng, encoding)
        rows.append([day, name_cell(rng, patient, encoding), rng.choice(["Consultation G", "Consultation longue", "Visite", "ECG", "Vaccination", "Certificat"]), rng.choice([30.0, 46.5, 60.0, 25.0, 15.0]), rng.choice(["Carte", "Espèces", "Virement", "Chèque"])])
    return [Sheet("Honoraires", ["Date", "Patient", "Acte", "Honoraires", "Mode de paiement"], rows)], [], pool.doctor_label(doctor)


def build_formation(rng, pool, year, month, module, encoding):
    rows = []
    for person in rng.sample(pool.staff, k=min(len(pool.staff), rng.randint(5, 10))) * 1:
        rows.append([f"{person.given} {person.family}" if person.role != "owner" else person.canonical, rng.choice(["Secrétaire", "Médecin", "Infirmière", "Assistante"]), dt.date(year, month, rng.randint(1, 28)), rng.choice(["oui", "oui", "non"]), float(rng.choice([2, 3.5, 7]))])
    return [Sheet("Présences", ["Participant", "Fonction", "Date", "Présent", "Heures"], rows)], [], module


def build_livraisons(rng, pool, year, month, supplier, encoding):
    rows = []
    for day in weekday_dates(rng, year, month, rng.randint(10, 35)):
        rows.append([day, supplier.canonical, rng.choice(["Gants", "Compresses", "Papier", "Toner", "Désinfectant", "Seringues"]), rng.randint(1, 60), rng.choice(["livré", "livré", "en attente", "retard"])])
    return [Sheet("Livraisons", ["Date", "Fournisseur", "Article", "Quantité", "Statut"], rows)], [supplier.id], supplier.canonical


# --------------------------------------------------------------------------------------------------- layout


def pick_format(rng: random.Random, index: int) -> str:
    # A fixed rotation, so every stretch of the stream holds all three.
    return ["xlsx", "csv", "xlsx", "ods", "csv", "xlsx", "ods", "xlsx", "csv", "ods"][index % 10]


def apply_traps(rng: random.Random, workbook: Workbook, index: int) -> None:
    """Decide the traps of one workbook from its position, so a slice shows each of them too."""
    table = workbook.table()
    numeric = [c for c in table.columns if c in ("Montant", "Montant TTC", "Honoraires", "Débit", "Montant annuel", "Prix unitaire")]
    traps = []
    if index % 7 == 3:
        table.title = f"Export du {rng.randint(1, 28):02d}/{rng.randint(1, 12):02d}/2026 - {workbook.family}"
        traps.append("title_row")
        if workbook.fmt != "csv" and index % 14 == 3:
            table.merged_title = True
            traps.append("merged_title")
    if workbook.fmt != "csv" and index % 8 == 5 and numeric:
        table.formula_total = numeric[0]
        traps.append("formula_total")
    if index % 11 == 6 and len(table.columns) >= 4:
        table.empty_column = 2
        traps.append("empty_column")
    if workbook.fmt != "csv" and index % 10 == 7:
        workbook.sheets.append(Sheet("Notes", ["Notes"], [["Liste tenue à jour par le secrétariat."], ["À vérifier avant la fin du trimestre."]], notes=True))
        traps.append("notes_sheet")
    if workbook.fmt == "csv":
        workbook.numbers = "ch" if index % 3 == 0 else "fr"
        workbook.dates = "ch" if workbook.numbers == "ch" else "fr"
        if workbook.numbers == "ch":
            traps.append("swiss_numbers")
        traps.append("text_dates")
        workbook.encoding = "cp1252" if index % 2 == 0 else "utf-8"
    elif index % 9 == 1:
        traps.append("text_dates")
    workbook.traps = traps


def grid_of(sheet: Sheet, textual_dates: bool) -> S.Grid:
    """The sheet as written: a title and a blank line above when it has a title, an empty column when it has one,
    a SUM formula in the last row of its amount column when it has one."""
    columns = list(sheet.columns)
    rows = [list(r) for r in sheet.rows]
    if sheet.empty_column is not None:
        columns.insert(sheet.empty_column, None)
        rows = [r[: sheet.empty_column] + [None] + r[sheet.empty_column:] for r in rows]
    width = len(columns)
    grid_rows = []
    merges = []
    if sheet.title:
        grid_rows.append([sheet.title] + [None] * (width - 1))
        grid_rows.append([None] * width)
        if sheet.merged_title:
            merges.append(f"A1:{S.column_letter(width - 1)}1")
    grid_rows.append(columns)
    first = len(grid_rows) + 1
    grid_rows += rows
    if sheet.formula_total:
        column = columns.index(sheet.formula_total)
        letter = S.column_letter(column)
        last = len(grid_rows)
        cached = total_cents(r[column] for r in rows if isinstance(r[column], float)) / 100
        total_row = [None] * width
        total_row[0] = "Total"
        total_row[column] = S.Formula(f"SUM({letter}{first}:{letter}{last})", cached)
        grid_rows.append(total_row)
    if textual_dates:
        grid_rows = [[S.date_text(c, "fr") if isinstance(c, dt.date) else c for c in r] for r in grid_rows]
    return S.Grid(sheet.name, grid_rows, merges)


def write(workbook: Workbook, root: Path) -> None:
    target = root / workbook.path
    target.parent.mkdir(parents=True, exist_ok=True)
    textual = "text_dates" in workbook.traps and workbook.fmt != "csv"
    grids = []
    for sheet in workbook.sheets:
        if sheet.notes:
            grids.append(S.Grid(sheet.name, [[sheet.name]] + sheet.rows, []))
        else:
            grids.append(grid_of(sheet, textual))
    if workbook.fmt == "xlsx":
        S.write_xlsx(target, grids)
    elif workbook.fmt == "ods":
        S.write_ods(target, grids)
    else:
        S.write_csv(target, grids[0], ";", workbook.encoding, workbook.numbers, workbook.dates)


# ------------------------------------------------------------------------------------------------ the library


def build_workbooks(cast: Cast, seed: int, out: Path, invoices: list, contracts: list, scale: float = 1.0) -> list:
    """The workbooks of the library, written under `out`. `scale` shrinks every family (the slice)."""
    pool = Pool(cast)
    books: list = []
    used: set = set()
    counter = {"n": 0}

    def make(family: str, period: str, entity: str, builder, *args, fmt: str | None = None) -> None:
        index = counter["n"]
        counter["n"] += 1
        rng = random.Random(f"wb-{seed}-{index}")
        chosen = fmt or pick_format(rng, index)
        encoding = "cp1252" if chosen == "csv" and index % 2 == 0 else "utf-8"
        sheets, people, label = builder(rng, pool, *args, encoding)
        stem = f"{family}-{period}-{slug(label or entity)}".strip("-")
        path = f"{stem}.{chosen}"
        counter_suffix = 2
        while path in used:
            path = f"{stem}-{counter_suffix}.{chosen}"
            counter_suffix += 1
        used.add(path)
        workbook = Workbook(path, chosen, family, period, entity or label, sheets, people=sorted(set(people)))
        apply_traps(rng, workbook, index)
        books.append(workbook)

    def how_many(family: str) -> int:
        return max(2, round(QUOTAS[family] * scale))

    sites = [
        {"name": "cabinet des Lilas", "country": "FR"}, {"name": "cabinet du Parc", "country": "FR"},
        {"name": "cabinet de Nyon", "country": "CH"}, {"name": "cabinet de Morges", "country": "CH"},
        {"name": "cabinet de Thonon", "country": "FR"}, {"name": "cabinet de Vevey", "country": "CH"},
    ]
    n = how_many("rendez-vous")
    for i in range(n):
        month = i % 12 + 1
        site = sites[(i // 12) % len(sites)]
        make("rendez-vous", f"2026-{month:02d}", site["name"], build_rendez_vous, 2026, month, site)

    by_supplier: dict = {}
    for inv in invoices:
        by_supplier.setdefault((inv["supplier"], (inv["date"].month - 1) // 3 + 1), []).append(inv)
    groups = sorted(by_supplier.items())
    n = how_many("factures-fournisseurs")
    for i in range(n):
        if i < len(groups):
            (supplier_id, quarter), group = groups[i]
            supplier = cast.by_id[supplier_id]
        else:
            supplier = pool.suppliers[i % len(pool.suppliers)]
            quarter = i % 4 + 1
            rng = random.Random(f"extra-invoices-{seed}-{i}")
            group = []
            for k in range(rng.randint(6, 20)):
                ht = round(rng.uniform(40, 1900), 2)
                group.append({"supplier": supplier.id, "number": f"FAC-2026-{800 + i * 30 + k:03d}", "date": dt.date(2026, (quarter - 1) * 3 + rng.randint(1, 3), rng.randint(1, 28)), "ht": ht, "vat": ht * 0.2})
        make("factures-fournisseurs", f"2026-t{quarter}", supplier.canonical, build_factures, 2026, quarter, supplier, group)

    categories = [{"name": "consommables", "country": "FR"}, {"name": "bureau", "country": "CH"}, {"name": "pharmacie", "country": "FR"}]
    for i in range(how_many("stock")):
        category = categories[i % 3]
        make("stock", f"2026-{i % 12 + 1:02d}", category["name"], build_stock, 2026, i % 12 + 1, category)
    teams = [{"name": "accueil"}, {"name": "soins"}, {"name": "secretariat"}]
    for i in range(how_many("planning-heures")):
        team = teams[i % 3]
        make("planning-heures", f"2026-{i % 12 + 1:02d}", team["name"], build_planning, 2026, i % 12 + 1, team)

    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    for i in range(how_many("patients-ch")):
        letters = set(alphabet[(i * 2) % 26: (i * 2) % 26 + 2 + (i % 3)])
        make("patients-ch", "liste", "-".join(sorted(letters)), build_patients_ch, letters)
    for i in range(how_many("patients-fr")):
        letters = set(alphabet[(i * 2 + 1) % 26: (i * 2 + 1) % 26 + 2 + (i % 3)])
        make("patients-fr", "liste", "-".join(sorted(letters)), build_patients_fr, letters)

    contract_groups = [contracts[k: k + 4] for k in range(0, max(len(contracts), 1), 4)] or [[]]
    for i in range(how_many("contrats")):
        group = contract_groups[i % len(contract_groups)]
        make("contrats", "2026", f"lot-{i + 1}", build_contrats, 2026, 1, f"lot-{i + 1}", group)
    for i in range(how_many("compta-depenses")):
        make("compta-depenses", f"2026-{i % 12 + 1:02d}", f"site-{i // 12 + 1}", build_compta, 2026, i % 12 + 1, f"site-{i // 12 + 1}")
    for i in range(how_many("honoraires")):
        doctor = pool.doctors[(i // 12) % len(pool.doctors)]
        make("honoraires", f"2026-{i % 12 + 1:02d}", doctor.family, build_honoraires, 2026, i % 12 + 1, doctor)
    for i in range(how_many("formation-presences")):
        make("formation-presences", f"2026-{i % 12 + 1:02d}", f"module-{i + 1}", build_formation, 2026, i % 12 + 1, f"module-{i + 1}")
    for i in range(how_many("livraisons")):
        supplier = pool.suppliers[i % len(pool.suppliers)]
        make("livraisons", f"2026-{i % 12 + 1:02d}", supplier.canonical, build_livraisons, 2026, i % 12 + 1, supplier)

    for workbook in books:
        write(workbook, out)
    return books
