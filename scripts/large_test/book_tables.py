"""The 100 questions on spreadsheets (10 in the slice). Each is asked of one workbook, by its file name, with that
workbook ticked, so the road is the tabular engine's. The truths are computed from the Python rows the files were
written from. See `book.py` for the shape of a question."""

from __future__ import annotations

import datetime as dt
import statistics
from collections import Counter, defaultdict

from knowledge_sandbox.cast import fold

from . import book as B
from . import workbooks as WB

CAPS_FULL = {
    "T01": 8, "T02": 28, "T03": 6, "T04": 10, "T05": 5, "T06": 12, "T07": 6, "T08": 3, "T09": 2, "T10": 4,
    "T11": 2, "T12": 3, "T13": 2, "T14": 3, "T18": 2, "T19": 2, "T20": 3, "T21": 1,
}
CAPS_SLICE = {"T01": 1, "T02": 3, "T03": 1, "T04": 1, "T06": 1, "T08": 1, "T10": 1, "T12": 1}
MEASURES = ["Montant", "Montant TTC", "Honoraires", "Débit", "Prix unitaire", "Montant annuel", "Heures", "Quantité"]
GROUPS = ["Médecin", "Motif", "Acte", "Catégorie", "Fournisseur", "Statut", "Mode de paiement", "Poste", "Fonction", "Libellé"]
WEEKDAYS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"]
ROAD_VALUE = ["tabular:value"]


# --------------------------------------------------------------------------------------------- the workbooks


class Pool:
    def __init__(self, b: B.Builder):
        self.b = b
        self.books = b.books

    def mix(self, workbooks: list) -> list:
        """The same workbooks, one family after another in turn, so that no family fills the block."""
        families: dict = defaultdict(list)
        for wb in workbooks:
            families[wb.family].append(wb)
        out = []
        for position in range(max((len(v) for v in families.values()), default=0)):
            for family in sorted(families):
                if position < len(families[family]):
                    out.append(families[family][position])
        return out

    def tables(self, wb) -> list:
        return [s for s in wb.sheets if not s.notes]

    def clean(self, wb) -> bool:
        """One table sheet, nothing else, at least eight rows: the engine needs no help to read it."""
        return len(wb.sheets) == 1 and len(wb.sheets[0].rows) >= 8 and not wb.sheets[0].formula_total and wb.usable()

    def named(self, wb, sheet) -> str:
        """How the question points at the sheet: by file, plus the sheet when there are several."""
        if len(wb.sheets) == 1:
            return f" dans le fichier {wb.path}"
        return f" dans le fichier {wb.path} (feuille {sheet.name})"

    def selection(self, wb) -> dict:
        return {"documents": "none", "workbooks": {"paths": [wb.path]}}

    def numeric(self, sheet) -> list:
        out = []
        for name in MEASURES:
            if name in sheet.columns:
                i = sheet.columns.index(name)
                values = [r[i] for r in sheet.rows if r[i] is not None]
                if len(values) >= 8 and all(isinstance(v, (int, float)) for v in values):
                    out.append(name)
        return out

    def values(self, sheet, name) -> list:
        i = sheet.columns.index(name)
        return [r[i] for r in sheet.rows if r[i] is not None]

    def texts(self, sheet) -> list:
        out = []
        for name in GROUPS:
            if name in sheet.columns:
                i = sheet.columns.index(name)
                values = [r[i] for r in sheet.rows if r[i] is not None]
                distinct = set(values)
                if values and all(isinstance(v, str) for v in values) and 2 <= len(distinct) <= 12:
                    out.append(name)
        return out

    def dates(self, sheet) -> str | None:
        for name in sheet.columns:
            if name.startswith("Date") or name.startswith("Dernière"):
                i = sheet.columns.index(name)
                if all(isinstance(r[i], dt.date) for r in sheet.rows if r[i] is not None):
                    return name
        return None


def cents_sum(values) -> float:
    return sum(WB.cents(v) for v in values) / 100


def tolerance(kind: str) -> float:
    return 0.01 if kind in ("mean", "median") else 0.005


# ------------------------------------------------------------------------------------------ generators


def aggregates(b: B.Builder, pool: Pool) -> None:
    """T02: sum, mean, median, minimum, maximum, rotating through the formats."""
    operations = ["sum", "mean", "max", "min", "median", "sum"]
    by_format = {fmt: pool.mix([wb for wb in pool.books if wb.fmt == fmt and pool.clean(wb)]) for fmt in ("xlsx", "ods", "csv")}
    turn = 0
    for round_number in range(40):
        for fmt in ("ods", "xlsx", "csv", "ods"):
            candidates = by_format[fmt]
            if not candidates:
                continue
            wb = candidates[(round_number * 3 + turn) % len(candidates)]
            turn += 1
            sheet = wb.sheets[0]
            measures = pool.numeric(sheet)
            if not measures:
                continue
            name = measures[turn % len(measures)]
            values = pool.values(sheet, name)
            kind = operations[turn % len(operations)]
            if kind == "sum":
                text, value = f"Quelle est la somme de {name}{pool.named(wb, sheet)} ?", cents_sum(values)
            elif kind == "mean":
                text, value = f"Quelle est la moyenne de {name}{pool.named(wb, sheet)} ?", round(cents_sum(values) / len(values), 2)
            elif kind == "max":
                text, value = f"Quel est le maximum de {name}{pool.named(wb, sheet)} ?", max(values)
            elif kind == "min":
                text, value = f"Quel est le minimum de {name}{pool.named(wb, sheet)} ?", min(values)
            else:
                text, value = f"Quelle est la médiane de {name}{pool.named(wb, sheet)} ?", round(statistics.median([WB.cents(v) for v in values]) / 100, 2)
            b.add("spreadsheets", "T02", text, expect={"value": value, "tolerance": tolerance(kind)}, route=ROAD_VALUE,
                  selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt, note=f"{wb.family}, {operation_label(kind)} of {name}")


def operation_label(kind: str) -> str:
    return {"sum": "sum", "mean": "mean", "max": "maximum", "min": "minimum", "median": "median"}[kind]


def structure(b: B.Builder, pool: Pool) -> None:
    """T01: columns, sheets, rows."""
    clean = pool.mix([wb for wb in pool.books if pool.clean(wb) and not wb.table().title])
    for k, wb in enumerate(clean[:12]):
        sheet = wb.sheets[0]
        if k % 3 == 0:
            b.add("spreadsheets", "T01", f"Quelles sont les colonnes du fichier {wb.path} ?",
                  expect={"columns": list(sheet.columns)}, route=["tabular:structural"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        elif k % 3 == 1:
            b.add("spreadsheets", "T01", f"Combien de lignes contient le fichier {wb.path} ?",
                  expect={"value": len(sheet.rows), "tolerance": 0}, route=["tabular:structural", "tabular:value"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        else:
            b.add("spreadsheets", "T01", f"Quelles sont les feuilles du fichier {wb.path} ?",
                  expect={"columns": [s.name for s in wb.sheets]}, route=["tabular:structural"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)


def filters(b: B.Builder, pool: Pool) -> None:
    """T03 (a value), T04 (dates), T05 (a comparison)."""
    clean = pool.mix([wb for wb in pool.books if pool.clean(wb) and not wb.table().title and len(wb.sheets) == 1])
    value_done = date_done = number_done = 0
    for k, wb in enumerate(clean):
        sheet = wb.sheets[0]
        texts = pool.texts(sheet)
        numbers = pool.numeric(sheet)
        date = pool.dates(sheet)
        if texts and value_done < 8:
            name = texts[0]
            i = sheet.columns.index(name)
            counts = Counter(r[i] for r in sheet.rows if isinstance(r[i], str) and not r[i].lower().startswith("en "))
            if counts:
                value, n = counts.most_common(1)[0]
                if n >= 2 and len(value.split()) <= 3:
                    b.add("spreadsheets", "T03", f"Combien de lignes avec {name} {value}{pool.named(wb, sheet)} ?", expect={"value": n, "tolerance": 0},
                          route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
                    value_done += 1
        if date and date_done < 12:
            i = sheet.columns.index(date)
            days = [r[i] for r in sheet.rows if isinstance(r[i], dt.date)]
            if not days:
                continue
            weekday = Counter(d.weekday() for d in days).most_common(1)[0][0]
            n = sum(1 for d in days if d.weekday() == weekday)
            style = k % 3
            if style == 0:
                text, count = f"Combien de lignes le {WEEKDAYS[weekday]}{pool.named(wb, sheet)} ?", n
            elif style == 1:
                month = days[0].month
                text, count = f"Combien de lignes en {WB.MONTHS[month - 1]}{pool.named(wb, sheet)} ?", sum(1 for d in days if d.month == month)
            else:
                low, high = min(days), min(days) + dt.timedelta(days=14)
                text = f"Combien de lignes entre le {low.day:02d}/{low.month:02d}/{low.year} et le {high.day:02d}/{high.month:02d}/{high.year}{pool.named(wb, sheet)} ?"
                count = sum(1 for d in days if low <= d <= high)
            if b.add("spreadsheets", "T04", text, expect={"value": count, "tolerance": 0}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt):
                date_done += 1
        if numbers and number_done < 8:
            name = numbers[0]
            values = pool.values(sheet, name)
            threshold = int(sorted(values)[len(values) // 2])
            n = sum(1 for v in values if v > threshold)
            if b.add("spreadsheets", "T05", f"Combien de lignes avec {name} supérieur à {threshold}{pool.named(wb, sheet)} ?", expect={"value": n, "tolerance": 0},
                     route=ROAD_VALUE + ["tabular:model_plan"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt):
                number_done += 1


def groups(b: B.Builder, pool: Pool) -> None:
    """T06 (the largest group, the total per group, the first three) and T07 (the different values)."""
    clean = pool.mix([wb for wb in pool.books if pool.clean(wb) and not wb.table().title and len(wb.sheets) == 1])
    done = 0
    for wb in clean:
        sheet = wb.sheets[0]
        texts, numbers = pool.texts(sheet), pool.numeric(sheet)
        if not texts or not numbers:
            continue
        group, measure = texts[0], numbers[0]
        gi, mi = sheet.columns.index(group), sheet.columns.index(measure)
        totals: dict = defaultdict(int)
        for r in sheet.rows:
            if isinstance(r[gi], str) and r[mi] is not None:
                totals[r[gi]] += WB.cents(r[mi])
        if len(totals) < 3:
            continue
        ordered = sorted(totals, key=lambda key: (-totals[key], key))
        ties = totals[ordered[0]] == totals[ordered[1]]
        if len(ordered) > 3:
            ties = ties or totals[ordered[2]] == totals[ordered[3]]
        if ties:
            continue
        kind = done % 4
        where = pool.named(wb, sheet)
        if kind == 0:
            b.add("spreadsheets", "T06", f"Quel {group} a le {measure} total le plus élevé{where} ?",
                  expect={"facts": [[ordered[0]]], "value": totals[ordered[0]] / 100, "tolerance": 0.005}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        elif kind == 1:
            b.add("spreadsheets", "T06", f"Quel est le total de {measure} par {group}{where} ?",
                  expect={"groups": {name: totals[name] / 100 for name in ordered}}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        elif kind == 2:
            b.add("spreadsheets", "T06", f"Quels sont les 3 premiers {group} par {measure}{where} ?",
                  expect={"ordered": ordered[:3]}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        else:
            b.add("spreadsheets", "T07", f"Liste les valeurs distinctes de {group}{where}",
                  expect={"facts": [[name] for name in sorted(totals)]}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt)
        done += 1


def traps(b: B.Builder, pool: Pool) -> None:
    """T08 to T14: the same questions on the files that carry a trap."""
    by_trap: dict = defaultdict(list)
    for wb in pool.books:
        if not wb.usable():
            continue
        for trap in wb.traps:
            by_trap[trap].append(wb)
    for trap in by_trap:
        by_trap[trap] = pool.mix(by_trap[trap])

    def sum_question(wb, feature, note):
        sheet = wb.table()
        measures = pool.numeric(sheet)
        if not measures:
            return None
        name = measures[0]
        where = pool.named(wb, sheet)
        value = cents_sum(pool.values(sheet, name))
        return b.add("spreadsheets", feature, f"Quelle est la somme de {name}{where} ?", expect={"value": value, "tolerance": 0.005}, route=ROAD_VALUE,
                     selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt, note=note)

    for wb in by_trap["title_row"]:
        if "formula_total" not in wb.traps and "merged_title" not in wb.traps:
            sum_question(wb, "T08", "the header is on row 3")
    for wb in by_trap["merged_title"]:
        if "formula_total" not in wb.traps:
            sum_question(wb, "T09", "the title is merged over the table")
    for wb in by_trap["empty_column"]:
        if "formula_total" not in wb.traps:
            sum_question(wb, "T11", "a column with nothing in it")
    # A column that ends with a SUM formula: the engine refuses that column, and totals another one.
    for wb in by_trap["formula_total"]:
        if len(wb.sheets) != 1:
            continue
        sheet = wb.table()
        column = sheet.formula_total
        if not column or sheet.notes:
            continue
        where = pool.named(wb, sheet)
        b.add("spreadsheets", "T10", f"Quelle est la somme de {column}{where} ?", expect={"nudge": "formula_cannot_be_verified"}, route=["tabular:nudge"],
              selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt, note="The last row of the column is a SUM formula; the engine will not total a column it cannot verify.")
    # A second sheet of free text: the engine asks which sheet.
    for wb in by_trap["notes_sheet"]:
        if "formula_total" in wb.traps or "title_row" in wb.traps:
            continue
        b.add("spreadsheets", "T12", f"Combien de lignes contient le fichier {wb.path} ?", expect={"nudge": "ambiguous_sheet_name"}, route=["tabular:nudge"],
              selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt, note="Two sheets, one of them free text: the engine asks, it does not pick.")
    for wb in by_trap["text_dates"]:
        sheet = wb.table()
        date = pool.dates(sheet)
        if wb.fmt != "csv" or not date or len(wb.sheets) != 1 or sheet.title or "formula_total" in wb.traps:
            continue
        i = sheet.columns.index(date)
        days = [r[i] for r in sheet.rows if isinstance(r[i], dt.date)]
        if not days:
            continue
        weekday = Counter(d.weekday() for d in days).most_common(1)[0][0]
        b.add("spreadsheets", "T13", f"Combien de lignes le {WEEKDAYS[weekday]} dans le fichier {wb.path} ?",
              expect={"value": sum(1 for d in days if d.weekday() == weekday), "tolerance": 0}, route=ROAD_VALUE, selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt,
              note=f"Dates written as text in the {wb.dates} style.")
    for wb in by_trap["swiss_numbers"]:
        if wb.table().title or len(wb.sheets) != 1:
            continue
        sum_question(wb, "T14", "numbers written 1'234.56 in a CSV")
    for wb in pool.books:
        if wb.fmt == "csv" and wb.numbers == "fr" and not wb.table().title and len(wb.sheets) == 1:
            sum_question(wb, "T14", f"numbers written 1 234,56 in a CSV ({wb.encoding})")


def honest(b: B.Builder, pool: Pool) -> None:
    """T18, T19, T20, T21: the questions the engine cannot or must not answer."""
    clean = [wb for wb in pool.books if pool.clean(wb) and len(wb.sheets) == 1]
    # Several workbooks ticked, none named.
    wb = clean[0]
    measure = pool.numeric(wb.sheets[0])[0] if pool.numeric(wb.sheets[0]) else "Montant"
    b.add("spreadsheets", "T18", f"Quelle est la somme de {measure} ?", expect={"nudge": "which_workbook"}, route=["tabular:which_workbook", "tabular:value"],
          selection={"documents": "none", "workbooks": "all"}, workbooks=[], fmt="",
          note="Every workbook is ticked and the question names none.")
    b.add("spreadsheets", "T18", "Combien de lignes ?", expect={"nudge": "which_workbook"}, route=["tabular:which_workbook"],
          selection={"documents": "none", "workbooks": "all"}, workbooks=[], fmt="")
    b.add("spreadsheets", "T19", "Quelle est la somme de Montant dans le fichier budget-previsionnel-2027.xlsx ?", expect={"nudge": "no_matching_file"},
          route=["tabular:no_matching_file"], selection={"documents": "none", "workbooks": "all"}, workbooks=[], fmt="")
    other = clean[-1]
    b.add("spreadsheets", "T19", f"Combien de lignes contient le fichier {other.path} ?", expect={"nudge": "file_not_selected"}, route=["tabular:file_not_selected"],
          selection={"documents": "none", "workbooks": {"paths": [clean[0].path]}}, workbooks=[other.path], fmt=other.fmt)
    # A value that is in no column, and one that is in two.
    for wb in clean:
        sheet = wb.sheets[0]
        texts = pool.texts(sheet)
        if texts:
            name = texts[0]
            b.add("spreadsheets", "T20", f"Combien de lignes avec {name} Zephyrine dans le fichier {wb.path} ?", expect={"nudge": "value_not_found"},
                  route=["tabular:nudge", "tabular:model_plan"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt, note="A value written nowhere in the table.")
            break
    # A word that is in two columns: a first name shared by a patient and a doctor of the same table.
    for wb in pool.books:
        sheet = wb.sheets[0]
        if not ("Patient" in sheet.columns and "Médecin" in sheet.columns and len(wb.sheets) == 1 and len(sheet.rows) >= 8 and wb.usable()):
            continue
        patient_column, doctor_column = sheet.columns.index("Patient"), sheet.columns.index("Médecin")
        patient_words = {w for row in sheet.rows for w in fold(str(row[patient_column])).split()}
        doctor_words = {w for row in sheet.rows for w in fold(str(row[doctor_column])).split() if w not in ("dr", "dre", "pr")}
        shared = sorted(patient_words & doctor_words)
        if shared:
            word = shared[0].capitalize()
            if b.add("spreadsheets", "T20", f"Combien de lignes pour {word} dans le fichier {wb.path} ?", expect={"nudge": "which_column"},
                     route=["tabular:which_column"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt,
                     note=f"{word} is written both in the patient column and in the doctor column: the engine asks which."):
                break
    # The hidden interpreter: a decimal threshold the classifier does not take.
    for wb in clean:
        sheet = wb.sheets[0]
        debit = [n for n in pool.numeric(sheet) if n in ("Débit", "Prix unitaire")]
        if debit and not wb.table().title:
            values = pool.values(sheet, debit[0])
            threshold = round(sorted(values)[len(values) // 2] + 0.02, 2)
            n = sum(1 for v in values if v > threshold)
            b.add("spreadsheets", "T21", f"Combien de lignes avec {debit[0]} supérieur à {threshold}{pool.named(wb, sheet)} ?", expect={"value": n, "tolerance": 0},
                  route=["tabular:value", "tabular:model_plan"], selection=pool.selection(wb), workbooks=[wb.path], fmt=wb.fmt,
                  note="A decimal threshold: in the probe the classifier did not read it and the model proposed the plan.")
            break


def add_all(b: B.Builder) -> None:
    b.feature_cap = CAPS_SLICE if b.slice else CAPS_FULL
    pool = Pool(b)
    structure(b, pool)
    traps(b, pool)
    honest(b, pool)
    filters(b, pool)
    groups(b, pool)
    aggregates(b, pool)
