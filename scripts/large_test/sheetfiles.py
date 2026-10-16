"""Writers for the spreadsheets of the `demo` profile: XLSX, ODS and CSV, with the things a real export has and the
sandbox's own writers do not: merged cells, real typed dates, formulas with their cached value, a title above the
header, an empty column, French and Swiss number and date styles.

A cell is `None`, a string, an int, a float, a `datetime.date`, or a `Formula`. Output is deterministic (fixed zip
dates, nothing reads the clock).
"""

from __future__ import annotations

import datetime as dt
from dataclasses import dataclass
from pathlib import Path
from xml.sax.saxutils import escape

from knowledge_sandbox import writers as W

EPOCH = dt.date(1899, 12, 30)


@dataclass(frozen=True)
class Formula:
    """A cell that holds an expression and the value it last computed."""

    expression: str
    cached: float


@dataclass
class Grid:
    """One sheet as written: rows of cells, and the merged ranges (`A1:F1`)."""

    name: str
    rows: list
    merges: list


def serial(day: dt.date) -> int:
    return (day - EPOCH).days


def column_letter(index: int) -> str:
    return W.column_letter(index)


# ------------------------------------------------------------------------------------------------------- XLSX


def write_xlsx(path: Path, grids: list[Grid]) -> None:
    overrides = "".join(
        f'<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        for n in range(1, len(grids) + 1)
    )
    content_types = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
        '<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>'
        f"{overrides}</Types>"
    )
    rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>'
    )
    sheet_rels = "".join(
        f'<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>'
        for n in range(1, len(grids) + 1)
    )
    workbook_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        f'{sheet_rels}<Relationship Id="rId{len(grids) + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>'
    )
    sheet_entries = "".join(
        f'<sheet name="{escape(grid.name)}" sheetId="{n}" r:id="rId{n}"/>' for n, grid in enumerate(grids, start=1)
    )
    workbook = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        f"<sheets>{sheet_entries}</sheets></workbook>"
    )
    styles = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        '<fonts count="1"><font/></fonts><fills count="1"><fill/></fills><borders count="1"><border/></borders>'
        '<cellStyleXfs count="1"><xf numFmtId="0"/></cellStyleXfs>'
        '<cellXfs count="2"><xf numFmtId="0" xfId="0"/><xf numFmtId="14" xfId="0" applyNumberFormat="1"/></cellXfs>'
        "</styleSheet>"
    )
    entries = [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", rels),
        ("xl/_rels/workbook.xml.rels", workbook_rels),
        ("xl/workbook.xml", workbook),
        ("xl/styles.xml", styles),
    ]
    for n, grid in enumerate(grids, start=1):
        row_xml = []
        for row_index, row in enumerate(grid.rows, start=1):
            cells = []
            for col_index, cell in enumerate(row):
                ref = f"{column_letter(col_index)}{row_index}"
                if cell is None or cell == "":
                    continue
                if isinstance(cell, dt.date):
                    cells.append(f'<c r="{ref}" s="1"><v>{serial(cell)}</v></c>')
                elif isinstance(cell, Formula):
                    cells.append(f'<c r="{ref}"><f>{escape(cell.expression)}</f><v>{cell.cached}</v></c>')
                elif isinstance(cell, (int, float)):
                    cells.append(f'<c r="{ref}"><v>{cell}</v></c>')
                else:
                    cells.append(f'<c r="{ref}" t="inlineStr"><is><t xml:space="preserve">{escape(str(cell))}</t></is></c>')
            row_xml.append(f'<row r="{row_index}">{"".join(cells)}</row>')
        merges = ""
        if grid.merges:
            merges = f'<mergeCells count="{len(grid.merges)}">' + "".join(f'<mergeCell ref="{ref}"/>' for ref in grid.merges) + "</mergeCells>"
        entries.append((
            f"xl/worksheets/sheet{n}.xml",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
            f'<sheetData>{"".join(row_xml)}</sheetData>{merges}</worksheet>',
        ))
    W._zip(path, entries)


# -------------------------------------------------------------------------------------------------------- ODS


def _number(value) -> str:
    return repr(round(float(value), 6)).rstrip("0").rstrip(".") if isinstance(value, float) else str(value)


def _ods_cell(cell, column: int, grid: Grid, row_index: int) -> str:
    span = ""
    for ref in grid.merges:
        start, end = ref.split(":")
        if start == f"{column_letter(column)}{row_index}":
            width = _column_of(end) - column + 1
            span = f' table:number-columns-spanned="{width}"'
    if cell is None or cell == "":
        return f"<table:table-cell{span}/>"
    if isinstance(cell, dt.date):
        shown = f"{cell.day:02d}/{cell.month:02d}/{cell.year}"
        return f'<table:table-cell office:value-type="date" office:date-value="{cell.isoformat()}"{span}><text:p>{shown}</text:p></table:table-cell>'
    if isinstance(cell, Formula):
        formula = escape("of:=" + cell.expression, {'"': "&quot;"})
        return (
            f'<table:table-cell table:formula="{formula}" office:value-type="float" '
            f'office:value="{_number(cell.cached)}"{span}><text:p>{_number(cell.cached)}</text:p></table:table-cell>'
        )
    if isinstance(cell, (int, float)):
        return f'<table:table-cell office:value-type="float" office:value="{_number(cell)}"{span}><text:p>{_number(cell)}</text:p></table:table-cell>'
    return f'<table:table-cell office:value-type="string"{span}><text:p>{escape(str(cell))}</text:p></table:table-cell>'


def _column_of(reference: str) -> int:
    letters = "".join(ch for ch in reference if ch.isalpha())
    index = 0
    for ch in letters:
        index = index * 26 + (ord(ch) - ord("A") + 1)
    return index - 1


def write_ods(path: Path, grids: list[Grid]) -> None:
    tables = ""
    for grid in grids:
        body = ""
        for row_index, row in enumerate(grid.rows, start=1):
            covered = set()
            for ref in grid.merges:
                start, end = ref.split(":")
                if int("".join(ch for ch in start if ch.isdigit())) == row_index:
                    for col in range(_column_of(start) + 1, _column_of(end) + 1):
                        covered.add(col)
            cells = ""
            for col_index, cell in enumerate(row):
                if col_index in covered:
                    cells += "<table:covered-table-cell/>"
                else:
                    cells += _ods_cell(cell, col_index, grid, row_index)
            body += f"<table:table-row>{cells}</table:table-row>"
        tables += f'<table:table table:name="{escape(grid.name)}">{body}</table:table>'
    W._odf(path, "application/vnd.oasis.opendocument.spreadsheet", f"<office:body><office:spreadsheet>{tables}</office:spreadsheet></office:body>")


# -------------------------------------------------------------------------------------------------------- CSV


def number_text(value: float, style: str) -> str:
    """`fr`: 1 234,56 (a plain space, a decimal comma); `ch`: 1'234.56; `plain`: 1234.56."""
    text = f"{abs(value):.2f}"
    whole, cents = text.split(".")
    sign = "-" if value < 0 else ""
    if style == "plain":
        return f"{sign}{whole}.{cents}"
    groups = f"{int(whole):,}"
    if style == "fr":
        return f"{sign}{groups.replace(',', ' ')},{cents}"
    return f"{sign}{groups.replace(',', chr(39))}.{cents}"


def date_text(day: dt.date, style: str) -> str:
    if style == "iso":
        return day.isoformat()
    if style == "ch":
        return f"{day.day:02d}.{day.month:02d}.{day.year}"
    return f"{day.day:02d}/{day.month:02d}/{day.year}"


def write_csv(path: Path, grid: Grid, delimiter: str, encoding: str, numbers: str, dates: str) -> None:
    def text(cell) -> str:
        if cell is None:
            return ""
        if isinstance(cell, dt.date):
            return date_text(cell, dates)
        if isinstance(cell, Formula):
            return number_text(cell.cached, numbers)
        if isinstance(cell, float):
            return number_text(cell, numbers)
        return str(cell)

    rows = [[text(cell) for cell in row] for row in grid.rows]
    W.write_csv(path, rows, delimiter=delimiter, encoding=encoding)
