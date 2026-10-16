"""The 50 questions that need a document and a spreadsheet together (6 in the slice): a person named in a letter and
found in the rows of a workbook, an amount in an invoice compared with the same line of a workbook, and the
questions that show how the application routes a question when both kinds of file are ticked. See `book.py`."""

from __future__ import annotations

from collections import defaultdict

from knowledge_sandbox.cast import fold, person_forms

from . import book as B
from . import book_docs as BD

CAPS_FULL = {"M01": 18, "M02": 13, "M03": 8, "M04": 2, "M05": 4, "M06": 5}
CAPS_SLICE = {"M01": 1, "M02": 1, "M03": 1, "M04": 1, "M05": 1, "M06": 1}


def both(workbook_paths: list) -> dict:
    return {"documents": "all", "workbooks": {"paths": workbook_paths}}


def person_and_rows(b: B.Builder, index: BD.Index) -> None:
    """M01: a letter of a person and the rows of the same person in an appointment book."""
    appointments = [wb for wb in b.books if wb.family == "rendez-vous" and wb.usable() and len(wb.sheets) == 1 and not wb.table().title
                    and "formula_total" not in wb.traps and "notes_sheet" not in wb.traps]
    letters = [r for r in b.records if r["kind"] == "lettre_confrere" and index.unique(r) and r["readable"] and not r["ocr"]
               and "next_visit" in r["facts"] and b.cast.by_id[r["about"][0]].role == "patient"]
    for r in letters:
        pid = r["about"][0]
        name = person_forms(b.cast.by_id[pid])["full"]
        wanted = fold(name)
        for wb in appointments:
            sheet = wb.sheets[0]
            column = sheet.columns.index("Patient")
            rows = [row for row in sheet.rows if fold(str(row[column])) == wanted]
            # Nothing else in the table may carry a word of the name (a doctor of the same first name would make
            # the engine ask which column is meant).
            words = set(wanted.split())
            others = {w for row in sheet.rows for i, cell in enumerate(row) if i != column and isinstance(cell, str) for w in fold(cell).split()}
            if words & others:
                continue
            # The count is asked of the cells written exactly as the name is (the other spellings are other values).
            if rows and len(sheet.rows) >= 8:
                text = (f"D'après le courrier du confrère, quel est le prochain rendez-vous de {name}, "
                        f"et combien de lignes pour {name} dans le fichier {wb.path} ?")
                expect = {"date": BD.iso_of_french(r["facts"]["next_visit"]), "value": len(rows), "tolerance": 0, "sources_any": [r["path"]]}
                if b.add("mixed", "M01", text, expect=expect, route=["mixed_tier", "mixed_data_only:value"], selection=both([wb.path]),
                         documents=[r["path"]], workbooks=[wb.path], fmt=wb.fmt):
                    break


def invoice_against_table(b: B.Builder, index: BD.Index) -> None:
    """M02: the total of an invoice in a document, and the line of the same number in a workbook."""
    rows_of: dict = defaultdict(list)
    for wb in b.books:
        if wb.family != "factures-fournisseurs" or not wb.usable():
            continue
        sheet = wb.table()
        column = sheet.columns.index("N° facture")
        for row in sheet.rows:
            rows_of[row[column]].append((wb, sheet, row))
    for r in b.records:
        if r["kind"] != "facture_fournisseur" or not r["readable"] or r["ocr"] or "total_ttc" not in r["facts"]:
            continue
        number = next((m["text"] for m in r["mentions"] if m["form"] == "invoice"), None)
        supplier = next((m["entity"] for m in r["mentions"] if m["entity"].startswith("o-") and m["entity"] not in ("o-cabinet-voirons", "o-marchetti")), None)
        if supplier is None:
            continue
        name = b.cast.by_id[supplier].canonical
        # Two suppliers may write the same invoice number: the supplier says which one is meant.
        found = [item for item in rows_of.get(number, []) if item[0].entity == name]
        if len(found) != 1:
            continue
        wb, sheet, row = found[0]
        ttc = row[sheet.columns.index("Montant TTC")]
        text = f"Le total TTC de la facture {number} de {name} dans le document est-il le même que dans le fichier {wb.path} ?"
        document_total = BD.amount_of(r["facts"]["total_ttc"])
        expect = {"values": sorted({document_total, ttc}), "tolerance": 0.011, "sources_any": [r["path"]]}
        b.add("mixed", "M02", text, expect=expect, route=["mixed_tier"], selection=both([wb.path]), documents=[r["path"]], workbooks=[wb.path], fmt=wb.fmt)


def routing(b: B.Builder, index: BD.Index) -> None:
    """M03: with both kinds ticked, a question about one side is answered by that side."""
    letters = [r for r in b.records if r["kind"] == "lettre_confrere" and index.unique(r) and r["readable"] and not r["ocr"]
               and "next_visit" in r["facts"] and b.cast.by_id[r["about"][0]].role == "patient"]
    for r in letters[:4]:
        name = person_forms(b.cast.by_id[r["about"][0]])["full"]
        b.add("mixed", "M03", f"Quel est le prochain rendez-vous de {name} d'après le courrier du confrère ?",
              expect={"date": BD.iso_of_french(r["facts"]["next_visit"]), "sources_any": [r["path"]]}, route=["mixed_tier"],
              selection={"documents": "all", "workbooks": "all"}, documents=[r["path"]], fmt=r["format"],
              note="A question about a letter, with every workbook ticked: the documents answer it.")
    from .book_tables import Pool

    pool = Pool(b)
    clean = [wb for wb in b.books if pool.clean(wb) and not wb.table().title and len(wb.sheets) == 1]
    for wb in clean[2:12:3][:4]:
        sheet = wb.sheets[0]
        measures = pool.numeric(sheet)
        if not measures:
            continue
        value = sum(pool.values(sheet, measures[0]))
        value = round(value, 2)
        b.add("mixed", "M03", f"Quelle est la somme de {measures[0]} dans le fichier {wb.path} ?",
              expect={"value": value, "tolerance": 0.005}, route=["mixed_data_only:value"], selection=both([wb.path]), workbooks=[wb.path], fmt=wb.fmt,
              note="A question about a table, with every document ticked: the engine answers it alone.")


def folder_and_set(b: B.Builder) -> None:
    """M04 and M05: questions about the documents themselves, with the tables ticked."""
    analysed = [r for r in b.records if r["readable"]]
    scans = sum(1 for r in analysed if r["ocr"])
    b.add("mixed", "M04", "Combien de documents ai-je ?", expect={"count_range": [len(analysed) - scans, len(analysed)]}, route=["mixed_folder"],
          selection={"documents": "all", "workbooks": "all"})
    b.add("mixed", "M04", "Combien de fichiers y a-t-il dans le dossier ?", expect={"count": len(b.records)}, route=["mixed_folder"],
          selection={"documents": "all", "workbooks": "all"})
    for word in ("Genève", "Nyon", "Lausanne", "Annecy"):
        plain = b.docs_containing(word)
        if plain:
            # The word search of the tables reads the file name, the headers and the text cells of the tables the
            # engine takes for tables (an unusable workbook is not searched).
            sheets = sorted(
                wb.path for wb in b.books
                if wb.usable() and (B.has_phrase(B.fold_text(wb.path.rsplit(".", 1)[0]), word)
                                    or any(isinstance(c, str) and B.has_phrase(B.fold_text(c), word) for s in wb.sheets if not s.notes for row in s.rows for c in row))
            )
            b.add("mixed", "M05", f"Quels documents parlent de {word} ?", expect={"listing": plain, "listing_optional": b.docs_containing(word, with_ocr=True), "total": len(plain), "workbook_listing": sheets},
                  route=["mixed_set"], selection={"documents": "all", "workbooks": "all"})


def fill(b: B.Builder) -> None:
    """M06: a letter for each invoice, from the template and a workbook: a plan to approve."""
    template = next((r for r in b.records if r["kind"] == "modele"), None)
    if template is None:
        return
    for wb in b.books:
        if wb.family != "factures-fournisseurs" or not wb.usable() or len(wb.sheets) != 1 or wb.table().title or wb.fmt == "csv" and wb.numbers == "ch":
            continue
        b.add("mixed", "M06", f"Rédige un courrier de relance pour chaque facture du fichier {wb.path}",
              expect={"template": template["path"], "data_file": wb.path}, route=["fill_plan"], selection=both([wb.path]),
              documents=[template["path"]], workbooks=[wb.path], fmt=wb.fmt,
              note="Nothing is written: the answer is a plan with the template, the table and a preview.")


def add_all(b: B.Builder) -> None:
    b.feature_cap = CAPS_SLICE if b.slice else CAPS_FULL
    index = BD.Index(b)
    fill(b)
    folder_and_set(b)
    routing(b, index)
    invoice_against_table(b, index)
    person_and_rows(b, index)
