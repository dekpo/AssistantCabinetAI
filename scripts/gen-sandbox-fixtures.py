"""Generate the sandbox fixtures that need a real binary format.

Fictional content only, matching fixtures/gp-sandbox/README.md. Run once from the repo root:

    python scripts/gen-sandbox-fixtures.py

Not part of the product; a one-off tool to produce test fixtures that a text editor cannot
produce (PDF, DOCX). Uses only the standard library, deliberately, so nothing extra needs
installing on the pilot workstation or in CI.
"""

from __future__ import annotations

import textwrap
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INBOX = ROOT / "fixtures" / "gp-sandbox" / "inbox"


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

    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("[Content_Types].xml", content_types)
        archive.writestr("_rels/.rels", rels)
        archive.writestr("word/document.xml", document_xml)


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

    print("Fixtures written under", INBOX)


if __name__ == "__main__":
    main()
