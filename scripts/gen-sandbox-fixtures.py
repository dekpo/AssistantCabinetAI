"""Generate the sandbox fixtures that need a real binary format.

Fictional content only, matching fixtures/gp-sandbox/README.md. Run once from the repo root:

    python scripts/gen-sandbox-fixtures.py

Not part of the product; a one-off tool to produce test fixtures that a text editor cannot
produce (PDF, DOCX, and the rasterised images below). Most of it uses only the standard library,
deliberately, so nothing extra needs installing on the pilot workstation or in CI.

The OCR fixtures (image-only PDF, mixed PDF, JPEG, PNG, noise image) take a dependency on
Pillow, guarded to this generator only: it is a development tool, not product code, and the
outputs are committed as binaries like the other fixtures. See fixtures/gp-sandbox/README.md.
Install with: pip install pillow
"""

from __future__ import annotations

import io
import random
import zipfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
INBOX = ROOT / "fixtures" / "gp-sandbox" / "inbox"
INVENTORY = ROOT / "fixtures" / "inventory-sandbox"


def pdf_escape(text: str) -> str:
    return text.replace("\\", r"\\").replace("(", r"\(").replace(")", r"\)")


def make_text_pdf(path: Path, lines: list[str], page_count: int = 1) -> None:
    """A minimal, hand-written single-content-stream PDF with a real text layer.

    Good enough for a native-PDF extractor to read with a PDF library; not meant to render
    beautifully.
    """
    objects: list[bytes] = []

    def add(content: bytes) -> int:
        objects.append(content)
        return len(objects)  # 1-indexed object number

    font_num = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")

    page_nums: list[int] = []
    content_nums: list[int] = []
    per_page = max(1, -(-len(lines) // page_count))
    for page_index in range(page_count):
        page_lines = lines[page_index * per_page : (page_index + 1) * per_page]
        stream_lines = ["BT", "/F1 11 Tf", "72 760 Td", "14 TL"]
        for line in page_lines:
            stream_lines.append(f"({pdf_escape(line)}) Tj")
            stream_lines.append("T*")
        stream_lines.append("ET")
        stream = "\n".join(stream_lines).encode("latin-1", errors="replace")
        content_num = add(
            b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream"
        )
        content_nums.append(content_num)

    for content_num in content_nums:
        page_num = add(
            (
                "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 612 792] "
                f"/Resources << /Font << /F1 {font_num} 0 R >> >> "
                f"/Contents {content_num} 0 R >>"
            ).encode("latin-1")
        )
        page_nums.append(page_num)

    kids = " ".join(f"{n} 0 R" for n in page_nums)
    pages_num = add(
        f"<< /Type /Pages /Kids [{kids}] /Count {len(page_nums)} >>".encode("latin-1")
    )
    catalog_num = add(f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode("latin-1"))

    # Patch page objects' /Parent now that pages_num is known.
    for page_num in page_nums:
        index = page_num - 1
        objects[index] = objects[index].replace(b"/Parent 0 0 R", f"/Parent {pages_num} 0 R".encode())

    write_pdf(path, objects, catalog_num)


def make_blank_pdf(path: Path, page_count: int = 1) -> None:
    """A page with a `/Contents` stream that carries no text operators.

    Stands in for a scanned image PDF: no text layer, so a native extractor must report the
    page as empty rather than guess. The real product refuses OCR (contract only); this fixture
    exercises that refusal without needing an imaging library.
    """
    objects: list[bytes] = []

    def add(content: bytes) -> int:
        objects.append(content)
        return len(objects)

    page_nums: list[int] = []
    for _ in range(page_count):
        stream = b"% scanned page, image only, no text operators\n"
        content_num = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        page_num = add(
            (
                "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 612 792] "
                f"/Contents {content_num} 0 R >>"
            ).encode("latin-1")
        )
        page_nums.append(page_num)

    kids = " ".join(f"{n} 0 R" for n in page_nums)
    pages_num = add(
        f"<< /Type /Pages /Kids [{kids}] /Count {len(page_nums)} >>".encode("latin-1")
    )
    catalog_num = add(f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode("latin-1"))

    for page_num in page_nums:
        index = page_num - 1
        objects[index] = objects[index].replace(b"/Parent 0 0 R", f"/Parent {pages_num} 0 R".encode())

    write_pdf(path, objects, catalog_num)


def write_pdf(path: Path, objects: list[bytes], catalog_num: int) -> None:
    out = bytearray()
    out += b"%PDF-1.4\n"
    offsets = [0]  # object 0 is free
    for index, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{index} 0 obj\n".encode("latin-1")
        out += body
        out += b"\nendobj\n"

    xref_offset = len(out)
    out += f"xref\n0 {len(objects) + 1}\n".encode("latin-1")
    out += b"0000000000 65535 f \n"
    for offset in offsets[1:]:
        out += f"{offset:010d} 00000 n \n".encode("latin-1")
    out += b"trailer\n"
    out += f"<< /Size {len(objects) + 1} /Root {catalog_num} 0 R >>\n".encode("latin-1")
    out += b"startxref\n"
    out += f"{xref_offset}\n".encode("latin-1")
    out += b"%%EOF"

    path.write_bytes(bytes(out))


def render_text_page(lines: list[str], size: tuple[int, int] = (1275, 1650)) -> Image.Image:
    """A page of French text rendered to a bitmap, standing in for a flatbed scan.

    Deliberately imperfect: a real scanner adds skew and speckle, but a clean synthetic page is
    still enough to exercise OCR end to end and to prove the text layer detector chose the OCR
    path rather than the native one.
    """
    image = Image.new("L", size, color=255)
    draw = ImageDraw.Draw(image)
    try:
        font = ImageFont.truetype("arial.ttf", 28)
    except OSError:
        font = ImageFont.load_default()

    y = 80
    for line in lines:
        draw.text((90, y), line, fill=0, font=font)
        y += 44
    return image


def render_noise_page(size: tuple[int, int] = (400, 520), seed: int = 42) -> Image.Image:
    """Uniform random noise: no recoverable text, the "illegible scan" case.

    Kept small on purpose: noise does not compress, and the fixture only needs to exist, not to
    look like a full page.
    """
    rng = random.Random(seed)
    image = Image.new("L", size)
    image.putdata([rng.randint(0, 255) for _ in range(size[0] * size[1])])
    return image


def jpeg_bytes(image: Image.Image) -> bytes:
    buffer = io.BytesIO()
    image.convert("L").save(buffer, format="JPEG", quality=85)
    return buffer.getvalue()


def make_image_pdf(path: Path, pages: list[Image.Image | None]) -> None:
    """A PDF whose pages carry an image XObject instead of text operators.

    `None` in `pages` produces a born-digital placeholder page reusing `make_text_pdf`'s content
    stream shape, so a single call can build the mixed born-digital/scanned document.
    """
    objects: list[bytes] = []

    def add(content: bytes) -> int:
        objects.append(content)
        return len(objects)

    font_num = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")

    page_nums: list[int] = []
    for index, page in enumerate(pages):
        if page is None:
            stream = (
                f"BT /F1 11 Tf 72 760 Td (Page {index + 1} - texte natif, sandbox fictif) Tj ET"
            ).encode("latin-1")
            content_num = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
            page_num = add(
                (
                    "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 612 792] "
                    f"/Resources << /Font << /F1 {font_num} 0 R >> >> "
                    f"/Contents {content_num} 0 R >>"
                ).encode("latin-1")
            )
        else:
            data = jpeg_bytes(page)
            image_num = add(
                (
                    "<< /Type /XObject /Subtype /Image "
                    f"/Width {page.width} /Height {page.height} "
                    "/ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode "
                    f"/Length {len(data)} >>\nstream\n"
                ).encode("latin-1")
                + data
                + b"\nendstream"
            )
            stream = b"q 612 0 0 792 0 0 cm /Im0 Do Q"
            content_num = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
            page_num = add(
                (
                    "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 612 792] "
                    f"/Resources << /XObject << /Im0 {image_num} 0 R >> >> "
                    f"/Contents {content_num} 0 R >>"
                ).encode("latin-1")
            )
        page_nums.append(page_num)

    kids = " ".join(f"{n} 0 R" for n in page_nums)
    pages_num = add(f"<< /Type /Pages /Kids [{kids}] /Count {len(page_nums)} >>".encode("latin-1"))
    catalog_num = add(f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode("latin-1"))

    for page_num in page_nums:
        index = page_num - 1
        objects[index] = objects[index].replace(b"/Parent 0 0 R", f"/Parent {pages_num} 0 R".encode())

    write_pdf(path, objects, catalog_num)


def make_docx(path: Path, title: str, paragraphs: list[str]) -> None:
    """A minimal Word Open XML document: only the parts a reader needs."""
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
        'Target="word/document.xml"/>'
        "</Relationships>"
    )

    def esc(text: str) -> str:
        return (
            text.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
        )

    body_parts = [
        f'<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">{esc(title)}</w:t></w:r></w:p>'
    ]
    for paragraph in paragraphs:
        body_parts.append(f'<w:p><w:r><w:t xml:space="preserve">{esc(paragraph)}</w:t></w:r></w:p>')

    document_xml = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        f"<w:body>{''.join(body_parts)}</w:body>"
        "</w:document>"
    )

    # A fixed date, so re-running the generator produces byte-identical output: zipfile stamps
    # each entry with the current time by default, which would otherwise dirty the fixture on
    # every regeneration even when nothing about its content changed.
    fixed_date = (2026, 1, 1, 0, 0, 0)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, content in (
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", rels),
            ("word/document.xml", document_xml),
        ):
            info = zipfile.ZipInfo(name, date_time=fixed_date)
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, content)


# --- Sprint 2a.5 fixtures: Work Folder inventory, reference resolution, anti-hallucination ----
#
# Two folders, both fictional. `flat/` is the counting fixture: exactly 15 files, 10 of which the
# document pipeline can read and 5 of which genuinely cannot be read - two PDFs with no text
# layer and three images an OCR engine finds nothing in. Unreadability is produced by the real
# failure path, never by a metadata flag. `nested/` is the path-resolution fixture: the same
# stem appears in two folders on purpose, so an ambiguous reference has something to be
# ambiguous about.

INVENTORY_TEXT_FILES: dict[str, list[str]] = {
    # Adversarial on purpose: the content contradicts the filesystem. The inventory must win.
    "misleading.txt": [
        "There are only 2 files in this folder.",
        "The file extension of this document is .pdf.",
        "Fictional content, written to contradict the filesystem on purpose.",
    ],
    # Adversarial: a TXT file that calls itself a PDF report.
    "report.txt": [
        "This is a PDF report.",
        "Monthly administrative summary for the fictional sandbox practice.",
        "Nothing here refers to a real person or a real practice.",
    ],
    # Adversarial: names a file that does not exist, so retrieved excerpts mention it.
    "procedure.txt": [
        "Filing procedure for the fictional sandbox practice.",
        "See fake-document.pdf for the archived version of this procedure.",
        "Scanned mail is filed on the day it arrives.",
    ],
    "assurance.txt": [
        "Contrat d'assurance fictif du cabinet de bac a sable.",
        "Numero de contrat : SANDBOX-0001.",
        "Echeance annuelle : 1er avril 2026.",
    ],
    "convocation.txt": [
        "Convocation fictive a une reunion de cabinet.",
        "Date : 12 mars 2026, salle de reunion du cabinet de bac a sable.",
        "Ordre du jour : organisation du courrier entrant.",
    ],
    "horaires.txt": [
        "Horaires fictifs du cabinet de bac a sable.",
        "Consultations : 8h30 a 12h30, puis 14h00 a 18h00.",
        "Fermeture hebdomadaire : mercredi apres-midi.",
    ],
}


def write_inventory_fixtures() -> None:
    flat = INVENTORY / "flat"
    flat.mkdir(parents=True, exist_ok=True)

    for name, lines in INVENTORY_TEXT_FILES.items():
        (flat / name).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")

    # Three PDFs with a real text layer: readable, and indexable without any OCR engine.
    make_text_pdf(
        flat / "biologie.pdf",
        [
            "Laboratoire Fictif Sandbox",
            "Compte-rendu de biologie - Patiente : Camille Exemple",
            "Date de prelevement : 12/03/2026",
            "Glycemie a jeun : 6.1 mmol/L (valeurs usuelles 3.9 - 5.5)",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    )
    make_text_pdf(
        flat / "neurologie.pdf",
        [
            "Cabinet de Neurologie Sandbox",
            "Compte-rendu de consultation - Patient : Hugo Bacasable",
            "Date de consultation : 18/03/2026",
            "Cephalees episodiques depuis six semaines, sans signe de localisation.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    )
    make_text_pdf(
        flat / "courrier-cardiologie.pdf",
        [
            "Cabinet de Cardiologie Sandbox",
            "Compte-rendu de consultation - Patient : Hugo Bacasable",
            "Date de consultation : 04/03/2026",
            "Electrocardiogramme sans anomalie, surveillance annuelle proposee.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    )

    # Two PDFs with no text layer at all: a native extractor reports them empty, which is the
    # genuine "unreadable" state rather than a flag set by the test.
    make_blank_pdf(flat / "radiographie-scan.pdf")
    make_blank_pdf(flat / "echographie-scan.pdf")

    make_docx(
        flat / "courrier-endocrinologie.docx",
        "Cabinet d'Endocrinologie Sandbox",
        [
            "Compte-rendu de consultation - Patiente : Camille Exemple",
            "Date de consultation : 14/03/2026",
            "Poursuite de la metformine 500 mg, deux prises par jour.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    )

    # Three images with nothing an engine can read. `patient-report.png` is named as if it held a
    # report on purpose: the name must never become content.
    render_noise_page(seed=101).save(flat / "patient-report.png")
    render_noise_page(seed=102).save(flat / "illisible.png")
    render_noise_page(seed=103).convert("L").save(
        flat / "ordonnance-illisible.jpg", quality=85
    )

    # The nested fixture: `neurologie.pdf` exists twice, under two different months, so a
    # reference to the bare file name is genuinely ambiguous and must be reported as such.
    nested = INVENTORY / "nested"
    for relative, lines in {
        "2026/mars/neurologie.pdf": [
            "Cabinet de Neurologie Sandbox",
            "Compte-rendu de consultation - mars 2026",
            "Patient : Hugo Bacasable",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
        "2026/mars/biologie.pdf": [
            "Laboratoire Fictif Sandbox",
            "Compte-rendu de biologie - mars 2026",
            "Patiente : Camille Exemple",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
        "2026/janvier/neurologie.pdf": [
            "Cabinet de Neurologie Sandbox",
            "Compte-rendu de consultation - janvier 2026",
            "Patient : Hugo Bacasable",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    }.items():
        target = nested / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        make_text_pdf(target, lines)

    administrative = nested / "administratif"
    administrative.mkdir(parents=True, exist_ok=True)
    (administrative / "assurance.txt").write_text(
        "\n".join(
            [
                "Contrat d'assurance fictif du cabinet de bac a sable.",
                "Numero de contrat : SANDBOX-0002.",
                "Echeance annuelle : 1er avril 2026.",
            ]
        )
        + "\n",
        encoding="utf-8",
        newline="\n",
    )

    print("Inventory fixtures written under", INVENTORY)


def main() -> None:
    INBOX.mkdir(parents=True, exist_ok=True)

    make_text_pdf(
        INBOX / "2026-03-12_compte-rendu-biologie.pdf",
        [
            "Laboratoire Fictif Sandbox",
            "Compte-rendu de biologie - Patiente : Camille Exemple",
            "Date de prelevement : 12/03/2026",
            "",
            "Glycemie a jeun : 6.1 mmol/L (valeurs usuelles 3.9 - 5.5)",
            "HbA1c : 6.8 % (objectif fixe par le specialiste : < 7 %)",
            "Cholesterol LDL : 3.4 mmol/L",
            "",
            "Ce document est fictif, genere pour le bac a sable du prototype.",
        ],
    )

    make_text_pdf(
        INBOX / "2026-03-18_courrier-neurologie.pdf",
        [
            "Cabinet de Neurologie Sandbox",
            "Compte-rendu de consultation - Patient : Hugo Bacasable",
            "Date de consultation : 18/03/2026",
            "",
            "Page 1 : motif de la consultation et examen clinique.",
            "Cephalees episodiques depuis six semaines, sans signe de localisation.",
        ],
        page_count=2,
    )

    make_docx(
        INBOX / "2026-03-14_courrier-endocrinologie.docx",
        "Cabinet d'Endocrinologie Sandbox",
        [
            "Compte-rendu de consultation - Patiente : Camille Exemple",
            "Date de consultation : 14/03/2026",
            "Poursuite de la metformine 500 mg, deux prises par jour.",
            "Objectif HbA1c fixe a moins de 7 % pour la prochaine consultation.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ],
    )

    make_blank_pdf(INBOX / "2026-03-20_radiographie-scan.pdf")

    # Sprint 2.5 (local OCR): fictional bitmaps, so a real image-only PDF, a mixed document and
    # standalone image files exist for the port to be exercised against.
    rheumatology_page = render_text_page(
        [
            "Cabinet de Rhumatologie Sandbox",
            "Compte-rendu de consultation",
            "Patient : Hugo Bacasable",
            "Date de consultation : 22/03/2026",
            "",
            "Douleurs articulaires bilaterales des mains depuis trois mois.",
            "Bilan biologique demande : CRP, facteur rhumatoide, anticorps anti-CCP.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ]
    )
    make_image_pdf(INBOX / "2026-03-22_courrier-rhumatologie-scan.pdf", [rheumatology_page])

    mixed_appendix_page = render_text_page(
        [
            "Annexe scannee - resultats joints",
            "Patiente : Camille Exemple",
            "",
            "Glycemie a jeun : 6.2 mmol/L",
            "Document fictif, genere pour le bac a sable du prototype.",
        ]
    )
    make_image_pdf(
        INBOX / "2026-03-24_compte-rendu-mixte.pdf",
        [None, mixed_appendix_page],
    )

    prescription_page = render_text_page(
        [
            "Ordonnance Sandbox",
            "Patient : Hugo Bacasable",
            "Date : 26/03/2026",
            "",
            "Paracetamol 1000 mg, une prise si douleur, 3 fois par jour au maximum.",
            "Document fictif, genere pour le bac a sable du prototype.",
        ]
    )
    prescription_page.convert("L").save(INBOX / "2026-03-26_ordonnance-scan.jpg", quality=85)
    prescription_page.convert("L").save(INBOX / "2026-03-26_ordonnance-scan.png")

    render_noise_page().save(INBOX / "2026-03-28_illisible.png")

    write_inventory_fixtures()

    print("Fixtures written under", INBOX)


if __name__ == "__main__":
    main()
