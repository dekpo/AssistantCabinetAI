"""File writers for the sandbox: every format a real library holds, standard library only (plus Pillow for
the pictures, exactly as `scripts/gen-sandbox-fixtures.py` does).

Output is deterministic: zip entries carry a fixed date, nothing reads the clock, nothing is random here.

A format the product does not read today (ODT, ODS, ODP, PPTX, VTT, RTF, EML, XLS, PPT) is written all the
same and as honestly as the standard library allows, so the day an adapter exists the files are there.
"""

from __future__ import annotations

import io
import struct
import textwrap
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

FIXED_DATE = (2026, 1, 1, 0, 0, 0)


# ------------------------------------------------------------------------------------------------ helpers


def can_cp1252(text: str) -> bool:
    try:
        text.encode("cp1252")
        return True
    except UnicodeEncodeError:
        return False


def _zip(path: Path, entries: list[tuple[str, str | bytes]], first_stored: str | None = None) -> None:
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, content in entries:
            info = zipfile.ZipInfo(name, date_time=FIXED_DATE)
            info.compress_type = zipfile.ZIP_STORED if name == first_stored else zipfile.ZIP_DEFLATED
            archive.writestr(info, content)


def _wrap(paragraphs: list[str], width: int) -> list[str]:
    lines: list[str] = []
    for paragraph in paragraphs:
        if not paragraph.strip():
            lines.append("")
            continue
        for line in paragraph.split("\n"):
            lines.extend(textwrap.wrap(line, width=width) or [""])
    return lines


# --------------------------------------------------------------------------------------- plain text family


def write_text(path: Path, title: str, paragraphs: list[str]) -> None:
    body = "\n\n".join(p for p in paragraphs)
    path.write_text((title + "\n\n" if title else "") + body + "\n", encoding="utf-8", newline="\n")


def write_markdown(path: Path, title: str, paragraphs: list[str]) -> None:
    body = "\n\n".join(paragraphs)
    path.write_text(f"# {title}\n\n{body}\n", encoding="utf-8", newline="\n")


def write_rtf(path: Path, title: str, paragraphs: list[str]) -> None:
    def rtf(text: str) -> str:
        out = []
        for ch in text:
            if ch in "\\{}":
                out.append("\\" + ch)
            elif ord(ch) > 127:
                out.append("\\u%d?" % (ord(ch) if ord(ch) < 32768 else ord(ch) - 65536))
            else:
                out.append(ch)
        return "".join(out)

    body = "".join(f"{rtf(p)}\\par\n" for p in [title, ""] + paragraphs)
    path.write_text("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Arial;}}\\f0\\fs22 " + body + "}", encoding="ascii", newline="\n")


def write_eml(path: Path, headers: dict, paragraphs: list[str]) -> None:
    lines = [f"{key}: {value}" for key, value in headers.items()]
    lines += ["MIME-Version: 1.0", "Content-Type: text/plain; charset=UTF-8", "Content-Transfer-Encoding: 8bit", ""]
    lines += ["\n\n".join(paragraphs)]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def write_vtt(path: Path, cues: list[tuple[str, str, str, str]]) -> None:
    """cues: (start, end, speaker, text) as `00:01:02.500`."""
    out = ["WEBVTT", "", "NOTE Transcription automatique d'une réunion (fictive)", ""]
    for number, (start, end, speaker, text) in enumerate(cues, start=1):
        out += [str(number), f"{start} --> {end}", f"<v {speaker}>{text}", ""]
    path.write_text("\n".join(out), encoding="utf-8", newline="\n")


def write_srt(path: Path, cues: list[tuple[str, str, str, str]]) -> None:
    """cues: (start, end, speaker, text) as `00:01:02.500`, written as SubRip: a number, `00:01:02,500 --> ...`,
    the words (the speaker leads the line, SubRip has no voice tag), a blank line. Windows line ends, as an
    export from a subtitle editor on Windows has them."""
    out = []
    for number, (start, end, speaker, text) in enumerate(cues, start=1):
        out += [str(number), f"{start.replace('.', ',')} --> {end.replace('.', ',')}", f"{speaker} : {text}", ""]
    path.write_bytes((chr(13) + chr(10)).join(out).encode("utf-8"))


# ------------------------------------------------------------------------------------------------- PDF


def _pdf_escape(text: str) -> str:
    return text.replace("\\", r"\\").replace("(", r"\(").replace(")", r"\)")


def _write_pdf_objects(path: Path, objects: list[bytes], catalog_num: int) -> None:
    out = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{index} 0 obj\n".encode("latin-1") + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n".encode("latin-1") + b"0000000000 65535 f \n"
    for offset in offsets[1:]:
        out += f"{offset:010d} 00000 n \n".encode("latin-1")
    out += f"trailer\n<< /Size {len(objects) + 1} /Root {catalog_num} 0 R >>\nstartxref\n{xref}\n%%EOF".encode("latin-1")
    path.write_bytes(bytes(out))


def write_pdf_text(path: Path, title: str, paragraphs: list[str], lines_per_page: int = 46, width: int = 92) -> int:
    """A PDF with a real text layer in WinAnsi (so accents read back). Returns the page count.
    The text must be encodable in CP1252: the caller picks another format otherwise."""
    lines = [title, ""] + _wrap(paragraphs, width) if title else _wrap(paragraphs, width)
    pages = [lines[i:i + lines_per_page] for i in range(0, len(lines), lines_per_page)] or [[""]]
    objects: list[bytes] = []

    def add(body: bytes) -> int:
        objects.append(body)
        return len(objects)

    font = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    contents: list[int] = []
    for page in pages:
        stream = ["BT", "/F1 11 Tf", "56 770 Td", "15 TL"]
        for line in page:
            stream.append(f"({_pdf_escape(line)}) Tj")
            stream.append("T*")
        stream.append("ET")
        data = "\n".join(stream).encode("cp1252")
        contents.append(add(b"<< /Length %d >>\nstream\n" % len(data) + data + b"\nendstream"))
    page_nums = [
        add(
            (
                "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 595 842] "
                f"/Resources << /Font << /F1 {font} 0 R >> >> /Contents {c} 0 R >>"
            ).encode("latin-1")
        )
        for c in contents
    ]
    kids = " ".join(f"{n} 0 R" for n in page_nums)
    pages_num = add(f"<< /Type /Pages /Kids [{kids}] /Count {len(page_nums)} >>".encode("latin-1"))
    catalog = add(f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode("latin-1"))
    for n in page_nums:
        objects[n - 1] = objects[n - 1].replace(b"/Parent 0 0 R", f"/Parent {pages_num} 0 R".encode())
    _write_pdf_objects(path, objects, catalog)
    return len(pages)


# --------------------------------------------------------------------------------------------- pictures


def _font(size: int):
    from PIL import ImageFont

    for name in ("arial.ttf", "Arial.ttf", "DejaVuSans.ttf", "LiberationSans-Regular.ttf", "Helvetica.ttc"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def scan_level(lines: list[str]) -> int:
    """How badly a page is scanned, 0 to 2, from its content so two runs agree: 0 a good copy, 1 a skewed and
    blurred one, 2 a photograph of a letter on a desk."""
    seed = sum(ord(c) * (i % 7 + 1) for i, c in enumerate("".join(lines))) % 1_000_003
    return seed % 3


_LEVELS = [
    # font, skew (degrees, either way), blur, scale, speckle divisor, ink, paper
    (26, 1.5, 0.9, 0.70, 500, 45, 250),
    (24, 2.5, 1.0, 0.62, 350, 60, 246),
    (23, 3.5, 1.2, 0.56, 250, 75, 242),
]


def render_page(lines: list[str], size: tuple[int, int] = (1275, 1650), noise: bool = False):
    """A page of text as a grey bitmap, standing in for a scan.

    With `noise` the page is made to look like a photocopier's or a phone's output instead of a screenshot:
    a skew, a blur, speckle, a shadow along one edge, a paler paper and a smaller size, in three degrees of
    badness chosen from the content (`scan_level`), which is what makes a real OCR engine miss a letter now and
    then. Without it the page is clean and every engine reads it perfectly, which proves nothing.
    """
    from PIL import Image, ImageDraw, ImageFilter

    level = scan_level(lines) if noise else None
    font_size, skew, blur, scale, speckle, ink, paper = _LEVELS[level] if noise else (28, 0, 0, 1, 1, 0, 255)
    image = Image.new("L", size, color=paper)
    draw = ImageDraw.Draw(image)
    font = _font(font_size)
    y = 90
    for line in lines:
        draw.text((100, y), line, fill=ink, font=font)
        y += int(font_size * 1.55)
    if not noise:
        return image
    import random

    seed = sum(ord(c) * (i % 7 + 1) for i, c in enumerate("".join(lines))) % 1_000_003
    rng = random.Random(seed)
    angle = (rng.random() - 0.5) * 2 * skew
    image = image.rotate(angle, resample=Image.BICUBIC, fillcolor=paper - 4)
    image = image.filter(ImageFilter.GaussianBlur(radius=blur))
    pixels = image.load()
    width, height = image.size
    for _ in range(width * height // speckle):
        x, y2 = rng.randrange(width), rng.randrange(height)
        pixels[x, y2] = rng.randint(60, 190)
    for x in range(0, 38):
        shade = int(30 * (1 - x / 38))
        for y2 in range(height):
            pixels[x, y2] = max(0, pixels[x, y2] - shade)
    return image.resize((int(width * scale), int(height * scale)), Image.LANCZOS)


def _jpeg(image) -> bytes:
    buffer = io.BytesIO()
    image.convert("L").save(buffer, format="JPEG", quality=50)
    return buffer.getvalue()


def write_image(path: Path, lines: list[str], fmt: str) -> None:
    image = render_page(lines, noise=True)
    if fmt == "png":
        image.save(path, format="PNG")
    else:
        path.write_bytes(_jpeg(image))


def write_pdf_scanned(path: Path, page_lines: list[list[str]]) -> int:
    """A PDF whose pages are pictures: no text layer, the OCR route."""
    objects: list[bytes] = []

    def add(body: bytes) -> int:
        objects.append(body)
        return len(objects)

    page_nums = []
    for lines in page_lines:
        image = render_page(lines, noise=True)
        data = _jpeg(image)
        image_num = add(
            (
                f"<< /Type /XObject /Subtype /Image /Width {image.width} /Height {image.height} "
                f"/ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode /Length {len(data)} >>\nstream\n"
            ).encode("latin-1") + data + b"\nendstream"
        )
        stream = b"q 595 0 0 842 0 0 cm /Im0 Do Q"
        content = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        page_nums.append(
            add(
                (
                    "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 595 842] "
                    f"/Resources << /XObject << /Im0 {image_num} 0 R >> >> /Contents {content} 0 R >>"
                ).encode("latin-1")
            )
        )
    kids = " ".join(f"{n} 0 R" for n in page_nums)
    pages_num = add(f"<< /Type /Pages /Kids [{kids}] /Count {len(page_nums)} >>".encode("latin-1"))
    catalog = add(f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode("latin-1"))
    for n in page_nums:
        objects[n - 1] = objects[n - 1].replace(b"/Parent 0 0 R", f"/Parent {pages_num} 0 R".encode())
    _write_pdf_objects(path, objects, catalog)
    return len(page_nums)


# ----------------------------------------------------------------------------------------- Office Open XML


def write_docx(path: Path, title: str, paragraphs: list[str]) -> None:
    content_types = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Override PartName="/word/document.xml" '
        'ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
        "</Types>"
    )
    rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" '
        'Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" '
        'Target="word/document.xml"/></Relationships>'
    )
    parts = []
    if title:
        parts.append(f'<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">{escape(title)}</w:t></w:r></w:p>')
    for paragraph in paragraphs:
        for line in paragraph.split("\n"):
            parts.append(f'<w:p><w:r><w:t xml:space="preserve">{escape(line)}</w:t></w:r></w:p>')
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        f"<w:body>{''.join(parts)}</w:body></w:document>"
    )
    _zip(path, [("[Content_Types].xml", content_types), ("_rels/.rels", rels), ("word/document.xml", document)])


def write_pptx(path: Path, title: str, slides: list[tuple[str, list[str]]]) -> None:
    ns = (
        'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"'
    )
    entries: list[tuple[str, str]] = []
    overrides = ['<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>']
    slide_ids, slide_rels = [], []
    for number, (heading, bullets) in enumerate(slides, start=1):
        paragraphs = "".join(
            f'<a:p><a:r><a:t>{escape(text)}</a:t></a:r></a:p>' for text in [heading] + bullets
        )
        entries.append(
            (
                f"ppt/slides/slide{number}.xml",
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                f"<p:sld {ns}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"
                '<p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Text"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/>'
                f"<p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp></p:spTree></p:cSld></p:sld>",
            )
        )
        overrides.append(
            f'<Override PartName="/ppt/slides/slide{number}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>'
        )
        slide_ids.append(f'<p:sldId id="{255 + number}" r:id="rId{number}"/>')
        slide_rels.append(
            f'<Relationship Id="rId{number}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{number}.xml"/>'
        )
    entries.insert(0, ("[Content_Types].xml",
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>' + "".join(overrides) + "</Types>"))
    entries.insert(1, ("_rels/.rels",
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>'))
    entries.append(("ppt/presentation.xml",
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f"<p:presentation {ns}><p:sldIdLst>{''.join(slide_ids)}</p:sldIdLst></p:presentation>"))
    entries.append(("ppt/_rels/presentation.xml.rels",
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' + "".join(slide_rels) + "</Relationships>"))
    _zip(path, entries)


def column_letter(index: int) -> str:
    letters = []
    n = index
    while True:
        letters.append(chr(ord("A") + n % 26))
        if n < 26:
            break
        n = n // 26 - 1
    return "".join(reversed(letters))


# A cell is str, int, float, ("date", serial), ("formula", expression, cached) or None (empty).
def write_xlsx(path: Path, sheets: list[tuple[str, list[list]]]) -> None:
    overrides = "".join(
        f'<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        for n in range(1, len(sheets) + 1)
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
        for n in range(1, len(sheets) + 1)
    )
    workbook_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        f'{sheet_rels}<Relationship Id="rId{len(sheets) + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>'
    )
    sheet_entries = "".join(
        f'<sheet name="{escape(name)}" sheetId="{n}" r:id="rId{n}"/>' for n, (name, _) in enumerate(sheets, start=1)
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
        "<fonts count=\"1\"><font/></fonts><fills count=\"1\"><fill/></fills><borders count=\"1\"><border/></borders>"
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
    for n, (_, rows) in enumerate(sheets, start=1):
        row_xml = []
        for row_index, row in enumerate(rows, start=1):
            cells = []
            for col_index, cell in enumerate(row):
                ref = f"{column_letter(col_index)}{row_index}"
                if cell is None or cell == "":
                    continue
                if isinstance(cell, tuple) and cell[0] == "date":
                    cells.append(f'<c r="{ref}" s="1"><v>{cell[1]}</v></c>')
                elif isinstance(cell, tuple) and cell[0] == "formula":
                    cells.append(f'<c r="{ref}"><f>{escape(cell[1])}</f><v>{cell[2]}</v></c>')
                elif isinstance(cell, (int, float)):
                    cells.append(f'<c r="{ref}"><v>{cell}</v></c>')
                else:
                    cells.append(f'<c r="{ref}" t="inlineStr"><is><t xml:space="preserve">{escape(str(cell))}</t></is></c>')
            row_xml.append(f'<row r="{row_index}">{"".join(cells)}</row>')
        entries.append(
            (
                f"xl/worksheets/sheet{n}.xml",
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
                f'<sheetData>{"".join(row_xml)}</sheetData></worksheet>',
            )
        )
    _zip(path, entries)


def write_csv(path: Path, rows: list[list], delimiter: str = ";", encoding: str = "utf-8", newline: str = "\r\n") -> None:
    def cell(value) -> str:
        text = "" if value is None else str(value)
        if delimiter in text or '"' in text or "\n" in text:
            text = '"' + text.replace('"', '""') + '"'
        return text

    text = newline.join(delimiter.join(cell(c) for c in row) for row in rows) + newline
    path.write_bytes(text.encode(encoding))


# --------------------------------------------------------------------------------------------- OpenDocument


_ODF_NS = (
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
    'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
    'xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" '
    'xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" '
    'xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" '
    'xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" '
    'xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"'
)


def _odf(path: Path, mimetype: str, body_xml: str) -> None:
    manifest = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">'
        f'<manifest:file-entry manifest:full-path="/" manifest:media-type="{mimetype}"/>'
        '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
        "</manifest:manifest>"
    )
    content = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f'<office:document-content {_ODF_NS} office:version="1.2">{body_xml}</office:document-content>'
    )
    _zip(path, [("mimetype", mimetype), ("META-INF/manifest.xml", manifest), ("content.xml", content)], first_stored="mimetype")


def write_odt(path: Path, title: str, paragraphs: list[str]) -> None:
    body = "".join(f"<text:p>{escape(p)}</text:p>" for p in ([title] if title else []) + [l for p in paragraphs for l in p.split("\n")])
    _odf(path, "application/vnd.oasis.opendocument.text", f"<office:body><office:text>{body}</office:text></office:body>")


def write_odp(path: Path, slides: list[tuple[str, list[str]]]) -> None:
    pages = ""
    for number, (heading, bullets) in enumerate(slides, start=1):
        items = "".join(f"<text:p>{escape(t)}</text:p>" for t in [heading] + bullets)
        pages += f'<draw:page draw:name="page{number}"><draw:frame svg:width="20cm" svg:height="12cm"><draw:text-box>{items}</draw:text-box></draw:frame></draw:page>'
    _odf(path, "application/vnd.oasis.opendocument.presentation", f"<office:body><office:presentation>{pages}</office:presentation></office:body>")


def write_ods(path: Path, sheets: list[tuple[str, list[list]]]) -> None:
    tables = ""
    for name, rows in sheets:
        body = ""
        for row in rows:
            cells = ""
            for cell in row:
                if cell is None or cell == "":
                    cells += "<table:table-cell/>"
                elif isinstance(cell, (int, float)):
                    cells += f'<table:table-cell office:value-type="float" office:value="{cell}"><text:p>{cell}</text:p></table:table-cell>'
                elif isinstance(cell, tuple) and cell[0] == "date":
                    cells += f'<table:table-cell office:value-type="float" office:value="{cell[1]}"><text:p>{cell[1]}</text:p></table:table-cell>'
                else:
                    cells += f'<table:table-cell office:value-type="string"><text:p>{escape(str(cell))}</text:p></table:table-cell>'
            body += f"<table:table-row>{cells}</table:table-row>"
        tables += f'<table:table table:name="{escape(name)}">{body}</table:table>'
    _odf(path, "application/vnd.oasis.opendocument.spreadsheet", f"<office:body><office:spreadsheet>{tables}</office:spreadsheet></office:body>")
