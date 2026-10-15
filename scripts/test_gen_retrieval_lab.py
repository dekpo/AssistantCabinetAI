"""Tests for scripts/gen-retrieval-lab.py. Run from scripts/: python -m unittest test_gen_retrieval_lab"""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("gen_retrieval_lab", Path(__file__).with_name("gen-retrieval-lab.py"))
gen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gen)


class SyntheticSet(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.out = Path(tempfile.mkdtemp()) / "set"
        cls.manifest = gen.build(cls.out, None)
        cls.documents = json.loads((cls.out / "documents.json").read_text(encoding="utf-8"))
        cls.questions = json.loads((cls.out / "questions.json").read_text(encoding="utf-8"))

    def test_the_sample_stays_small_enough_to_analyse_in_minutes(self):
        self.assertLessEqual(len(self.documents), 50)
        self.assertLessEqual(self.manifest["pages_total"], 80)

    def test_every_listed_document_exists_and_paths_are_clean(self):
        for document in self.documents:
            path = document["path"]
            self.assertTrue((self.out / "documents" / path).is_file(), path)
            self.assertRegex(path, r"^[A-Za-z0-9_./-]+$", "a clean file name: no space, no accent")

    def test_every_question_points_at_documents_of_the_set(self):
        known = {d["path"] for d in self.documents}
        for question in self.questions:
            self.assertTrue(set(question["relevant_readable"]) <= known, question["id"])

    def test_absent_questions_have_no_relevant_file_and_no_fact(self):
        absent = [q for q in self.questions if q["category"] == "absent"]
        self.assertGreaterEqual(len(absent), 8)
        for question in absent:
            self.assertEqual((question["relevant_documents"], question["expect"]["contains"]), ([], []))

    def test_a_paraphrase_question_does_not_contain_the_planted_fact(self):
        for question in self.questions:
            if question["category"] == "paraphrase":
                for fact in question["expect"]["contains"]:
                    self.assertNotIn(gen.squash(fact), gen.squash(question["text"]))

    def test_the_table_header_is_on_page_one_and_the_rows_on_page_two(self):
        lines = gen.table_lines()
        self.assertEqual(lines.index(gen.TABLE_HEADER) // gen.LINES_PER_PAGE + 1, 1)
        first_row = next(i for i, line in enumerate(lines) if line.startswith(gen.TABLE_ROWS[0][0]))
        self.assertEqual(first_row // gen.LINES_PER_PAGE + 1, 2)
        document = next(d for d in self.documents if d["path"] == gen.TABLE_PATH)
        self.assertEqual(document["pages"], 2)

    def test_a_normal_form_question_writes_the_date_or_amount_another_way_than_the_document(self):
        asked = [q for q in self.questions if q["category"] == "normal_form"]
        self.assertGreaterEqual(len(asked), 10)
        for question in asked:
            self.assertEqual(len(question["relevant_readable"]), 1)
            for fact in question["expect"]["contains"]:
                self.assertNotIn(gen.squash(fact), gen.squash(question["text"]), question["id"])

    def test_the_question_ids_are_unique(self):
        ids = [q["id"] for q in self.questions]
        self.assertEqual(len(ids), len(set(ids)))

    def test_the_output_is_deterministic(self):
        again = Path(tempfile.mkdtemp()) / "again"
        gen.build(again, None)
        for name in ("documents.json", "questions.json"):
            self.assertEqual((again / name).read_text(encoding="utf-8"), (self.out / name).read_text(encoding="utf-8"))

    def test_the_identifier_check_digit_of_the_made_up_number_is_valid(self):
        self.assertEqual(gen.nir_key(gen.NIR_BODY), f"{97 - (int(gen.NIR_BODY) % 97):02d}")


class BigSet(unittest.TestCase):
    """The templated documents, without the books (they are not in git)."""

    @classmethod
    def setUpClass(cls):
        cls.out = Path(tempfile.mkdtemp()) / "big"
        cls.manifest = gen.build(cls.out, None, 150)
        cls.documents = json.loads((cls.out / "documents.json").read_text(encoding="utf-8"))
        cls.questions = json.loads((cls.out / "questions.json").read_text(encoding="utf-8"))

    def test_the_set_is_filled_up_to_the_requested_size(self):
        self.assertEqual(len(self.documents), 150)
        self.assertEqual(len({d["path"] for d in self.documents}), 150, "no two documents share a path")

    def test_every_templated_question_points_at_one_existing_document(self):
        known = {d["path"] for d in self.documents}
        bulk_questions = [q for q in self.questions if q["relevant_documents"] and "/20" in q["relevant_documents"][0]]
        self.assertGreater(len(bulk_questions), 30)
        for question in bulk_questions:
            self.assertEqual(len(question["relevant_documents"]), 1)
            self.assertIn(question["relevant_documents"][0], known)

    def test_no_two_templated_documents_are_about_the_same_person(self):
        generator = gen.bulk.Bulk(7, set())
        documents = list(generator.documents(300))
        self.assertEqual(len(documents), 300)
        self.assertEqual(len(generator.names), 300)

    def test_the_avs_numbers_made_up_for_the_set_have_a_valid_check_digit(self):
        import random

        raw, dotted = gen.bulk.avs_number(random.Random(1))
        total = sum(int(d) * (3 if i % 2 else 1) for i, d in enumerate(raw[:12]))
        self.assertEqual(int(raw[12]), (10 - total % 10) % 10)
        self.assertEqual(dotted.replace(".", ""), raw)

    def test_the_big_set_is_deterministic(self):
        again = Path(tempfile.mkdtemp()) / "again"
        gen.build(again, None, 150)
        self.assertEqual((again / "questions.json").read_text(encoding="utf-8"),
                         (self.out / "questions.json").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
