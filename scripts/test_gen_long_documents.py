"""Tests for scripts/gen-long-documents.py. Run from scripts/: python -m unittest test_gen_long_documents

The corpus is generated small here (a few dozen pages); the real run uses 500, 1 000 and 2 000 pages. What is
checked is the ground truth: a planted fact must sit on the page, in the section and in the layout the
questions file claims, because the whole long-document programme is measured against it.
"""

import hashlib
import importlib.util
import json
import re
import tempfile
import unittest
import zipfile
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("gen_long_documents", Path(__file__).with_name("gen-long-documents.py"))
gen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gen)

SMALL = {"pdf": 40, "pdf_big": 60, "docx": 36, "md": 50, "txt": 50, "scan": 12}


def pdf_pages(path: Path) -> list:
    """The lines of each page of a PDF this generator wrote: one content stream per page, one `Tj` per line."""
    data = path.read_bytes().decode("latin-1")
    pages = []
    for stream in re.findall(r"stream\n(BT.*?ET)\nendstream", data, flags=re.S):
        lines = []
        for raw in re.findall(r"\((.*?)\) Tj", stream, flags=re.S):
            text = raw.replace(r"\(", "(").replace(r"\)", ")").replace(r"\\", "\\")
            lines.append(text.encode("latin-1").decode("cp1252"))
        pages.append(lines)
    return pages


def sha(folder: Path) -> dict:
    return {
        str(p.relative_to(folder)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(folder.rglob("*"))
        if p.is_file()
    }


class Corpus(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.out = Path(tempfile.mkdtemp()) / "long"
        cls.manifest = gen.build(cls.out, sizes=SMALL)
        read = lambda name: json.loads((cls.out / name).read_text(encoding="utf-8"))  # noqa: E731
        cls.documents = read("documents.json")
        cls.questions = read("questions.json")
        cls.sections = read("sections.json")
        cls.by_path = {d["path"]: d for d in cls.documents}

    # ------------------------------------------------------------------------------------ determinism

    def test_the_same_seed_gives_the_same_bytes(self):
        other = Path(tempfile.mkdtemp()) / "again"
        gen.build(other, sizes=SMALL)
        self.assertEqual(sha(self.out), sha(other))

    # ------------------------------------------------------------------------------------ the files

    def test_every_listed_document_exists_with_a_clean_name_and_the_page_count_it_claims(self):
        for document in self.documents:
            path = self.out / "documents" / document["path"]
            self.assertTrue(path.is_file(), document["path"])
            self.assertRegex(document["path"], r"^[A-Za-z0-9_./-]+$")
            if document["format"] == "pdf":
                self.assertEqual(len(pdf_pages(path)), document["pages"], document["path"])

    def test_the_corpus_has_every_format_and_size_class_the_programme_measures(self):
        formats = {d["format"] for d in self.documents}
        self.assertEqual(formats, {"pdf", "docx", "md", "txt", "scan-pdf"})
        self.assertGreaterEqual(len([d for d in self.documents if d["format"] == "pdf"]), 2)

    def test_every_pdf_page_has_the_same_running_header_and_a_numbered_footer(self):
        for document in self.documents:
            if document["format"] != "pdf":
                continue
            pages = pdf_pages(self.out / "documents" / document["path"])
            headers = {page[0] for page in pages}
            self.assertEqual(len(headers), 1, "one running header, repeated on every page")
            for number, page in enumerate(pages, start=1):
                self.assertEqual(page[-1], f"Page {number} / {len(pages)}")

    def test_the_pdf_outline_lists_the_sections_the_ground_truth_gives(self):
        for document in self.documents:
            if document["format"] != "pdf":
                continue
            raw = (self.out / "documents" / document["path"]).read_bytes().decode("latin-1")
            titles = re.findall(r"/Title \((.*?)\)", raw)
            expected = [s["label"] for s in self.sections[document["path"]] if s["level"] <= 2]
            self.assertEqual([t.encode("latin-1").decode("cp1252") for t in titles], expected)

    def test_the_docx_marks_its_headings_with_styles_and_the_markdown_with_hashes(self):
        for document in self.documents:
            path = self.out / "documents" / document["path"]
            levels = [s["level"] for s in self.sections[document["path"]]]
            if document["format"] == "docx":
                xml = zipfile.ZipFile(path).read("word/document.xml").decode("utf-8")
                self.assertEqual(len(re.findall(r'w:pStyle w:val="Heading1"', xml)), levels.count(1))
                self.assertEqual(len(re.findall(r'w:pStyle w:val="Heading2"', xml)), levels.count(2))
            if document["format"] == "md":
                text = path.read_text(encoding="utf-8")
                self.assertEqual(len(re.findall(r"^# ", text, flags=re.M)), levels.count(1) + 1)
                self.assertEqual(len(re.findall(r"^## ", text, flags=re.M)), levels.count(2))
            if document["format"] == "txt":
                text = path.read_text(encoding="utf-8")
                self.assertEqual(len(re.findall(r"^\d+\.\d+ ", text, flags=re.M)), levels.count(2))

    def test_the_scan_has_no_text_layer_and_the_truth_carries_the_text_of_each_page(self):
        scan = next(d for d in self.documents if d["format"] == "scan-pdf")
        raw = (self.out / "documents" / scan["path"]).read_bytes().decode("latin-1")
        self.assertNotIn(") Tj", raw)
        self.assertEqual(len(scan["ocr_pages"]), scan["pages"])
        self.assertTrue(scan["ocr"])

    # ------------------------------------------------------------------------------------ the planted facts

    def planted(self, kind):
        return [q for q in self.questions if q.get("plant") == kind]

    def test_every_kind_of_fact_the_programme_needs_is_planted_in_every_text_document(self):
        kinds = {q["plant"] for q in self.questions}
        self.assertEqual(
            kinds,
            {"prose", "heading", "list", "list_split", "table_split", "hyphen_line", "hyphen_page", "absent"},
        )

    def test_a_prose_fact_sits_on_the_page_the_question_claims(self):
        for q in self.planted("prose"):
            document = self.by_path[q["relevant_documents"][0]]
            if document["format"] != "pdf":
                continue
            pages = pdf_pages(self.out / "documents" / document["path"])
            page = q["fact_pages"][0]
            self.assertIn(q["expect"]["contains"][0], " ".join(pages[page - 1]).replace("-\n", ""))
            for other, lines in enumerate(pages, start=1):
                if other != page:
                    self.assertNotIn(q["expect"]["contains"][0], " ".join(lines))

    def test_a_table_is_split_with_its_header_on_the_last_body_line_of_a_page(self):
        for q in self.planted("table_split"):
            document = self.by_path[q["relevant_documents"][0]]
            if document["format"] != "pdf":
                continue
            pages = pdf_pages(self.out / "documents" / document["path"])
            header, row = q["expect"]["contains"]
            page = q["header_page"]
            self.assertEqual(pages[page - 1][-2], header, "the header is the last line before the footer")
            self.assertIn(row, pages[page])
            self.assertEqual(q["fact_pages"], [page + 1])

    def test_a_split_list_has_its_title_on_one_page_and_the_item_on_the_next(self):
        for q in self.planted("list_split"):
            document = self.by_path[q["relevant_documents"][0]]
            if document["format"] != "pdf":
                continue
            pages = pdf_pages(self.out / "documents" / document["path"])
            title, item = q["expect"]["contains"]
            page = q["header_page"]
            self.assertIn(title, pages[page - 1])
            self.assertNotIn(item, " ".join(pages[page - 1]))
            self.assertIn(item, " ".join(pages[page]))

    def test_a_hyphenated_term_is_cut_at_the_end_of_a_line_and_across_a_page_break(self):
        for q in self.planted("hyphen_line") + self.planted("hyphen_page"):
            document = self.by_path[q["relevant_documents"][0]]
            if document["format"] != "pdf":
                continue
            pages = pdf_pages(self.out / "documents" / document["path"])
            head, tail = q["hyphenated"]
            page = q["header_page"]
            self.assertTrue(pages[page - 1][-2].endswith(head + "-") or any(l.endswith(head + "-") for l in pages[page - 1]))
            if q["plant"] == "hyphen_page":
                self.assertTrue(pages[page - 1][-2].endswith(head + "-"))
                self.assertTrue(pages[page][1].startswith(tail))
            joined = "".join(head + tail)
            self.assertEqual(joined, q["term"])
            self.assertNotIn(q["term"], " ".join(" ".join(p) for p in pages), "the whole word never appears")

    def test_an_absent_fact_is_searched_for_on_a_topic_the_document_does_cover_and_appears_nowhere(self):
        absent = self.planted("absent")
        self.assertTrue(absent)
        everything = []
        for document in self.documents:
            path = self.out / "documents" / document["path"]
            if document["format"] == "pdf":
                everything += [" ".join(p) for p in pdf_pages(path)]
            elif document["format"] in ("md", "txt"):
                everything.append(path.read_text(encoding="utf-8"))
            elif document["format"] == "scan-pdf":
                everything += document["ocr_pages"]
        blob = " ".join(everything).casefold()
        for q in absent:
            self.assertEqual(q["relevant_documents"], [])
            self.assertEqual(q["relevant_readable"], [])
            self.assertEqual(q["category"], "absent")
            self.assertNotIn(q["absent_term"].casefold(), blob)
            self.assertIn(q["topic_term"].casefold(), blob, "the topic is covered, only the fact is not")

    def test_every_fact_of_a_text_document_has_a_section_label_in_the_ground_truth(self):
        for q in self.questions:
            if q["plant"] == "absent":
                continue
            labels = {s["label"] for s in self.sections[q["relevant_documents"][0]]}
            self.assertTrue(q["fact_sections"], q["id"])
            for label in q["fact_sections"]:
                self.assertIn(label, labels)

    # ------------------------------------------------------------------------------------ the shape the Rust lab reads

    def test_the_ground_truth_has_every_field_the_laboratory_deserialises(self):
        document_keys = {"index", "path", "format", "extension", "kind", "readable", "ocr", "pages", "tier"}
        question_keys = {"id", "category", "text", "selection", "expect", "min_tier", "relevant_documents", "relevant_readable"}
        for document in self.documents:
            self.assertLessEqual(document_keys, set(document))
        for q in self.questions:
            self.assertLessEqual(question_keys, set(q))
            self.assertIn("resolution", q["expect"])
        self.assertEqual(json.loads((self.out / "entities.json").read_text(encoding="utf-8")), [])
        self.assertEqual(json.loads((self.out / "workbooks.json").read_text(encoding="utf-8")), [])
        self.assertIn("seed", json.loads((self.out / "manifest.json").read_text(encoding="utf-8")))

    def test_the_text_of_the_question_never_contains_the_fact_it_asks_for(self):
        for q in self.questions:
            for fact in q["expect"]["contains"]:
                self.assertNotIn(fact.casefold(), q["text"].casefold(), q["id"])


class AddedToALibrary(unittest.TestCase):
    def test_the_long_documents_join_an_existing_library_after_its_last_document_and_change_nothing_else(self):
        base = Path(tempfile.mkdtemp()) / "base"
        (base / "documents" / "a").mkdir(parents=True)
        (base / "documents" / "a" / "one.txt").write_text("hello", encoding="utf-8")
        record = {"index": 1, "path": "a/one.txt", "format": "txt", "extension": "txt", "kind": "x", "readable": True,
                  "ocr": False, "pages": 1, "tier": 30}
        question = {"id": "Q001", "category": "term", "text": "t", "selection": {"documents": "all", "workbooks": "all"},
                    "expect": {"resolution": "none", "contains": []}, "min_tier": 30, "relevant_documents": [],
                    "relevant_readable": []}
        for name, value in (("documents.json", [record]), ("questions.json", [question]), ("entities.json", []),
                            ("workbooks.json", []), ("manifest.json", {"seed": 1, "documents": 1})):
            (base / name).write_text(json.dumps(value), encoding="utf-8")
        out = Path(tempfile.mkdtemp()) / "mixed"
        gen.build(out, sizes=SMALL, base=base)
        documents = json.loads((out / "documents.json").read_text(encoding="utf-8"))
        questions = json.loads((out / "questions.json").read_text(encoding="utf-8"))
        self.assertEqual(documents[0], record)
        self.assertEqual(questions[0], question)
        self.assertEqual([d["index"] for d in documents], list(range(1, len(documents) + 1)))
        self.assertTrue(all(d["tier"] > 1 for d in documents[1:]))
        self.assertTrue(all(q["min_tier"] > 1 for q in questions[1:]))
        self.assertEqual(len({q["id"] for q in questions}), len(questions), "question ids stay unique")
        self.assertTrue((out / "documents" / "a" / "one.txt").is_file())
        self.assertEqual(json.loads((out / "manifest.json").read_text(encoding="utf-8"))["documents"], len(documents))


if __name__ == "__main__":
    unittest.main()
