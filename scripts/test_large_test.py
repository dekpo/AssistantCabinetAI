"""Tests of the large test's generator, book and scorer.

    python -m unittest scripts.test_large_test                       (from the repository root)
    ACAI_SKIP_SLOW=1 python -m unittest scripts.test_large_test      (skips the full library, about 20 seconds)
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from large_test import score  # noqa: E402
from test_gen_knowledge_sandbox import tree_digest  # noqa: E402

NEW_DOCUMENT_FORMATS = ["vtt", "srt", "rtf", "eml", "odt", "odp", "pptx"]
READER_FEATURES = {"vtt": "D16", "srt": "D17", "rtf": "D18", "eml": "D19", "odt": "D20", "odp": "D21", "pptx": "D22"}


def generate(out: Path, *arguments: str) -> dict:
    subprocess.run([sys.executable, str(ROOT / "scripts" / "gen-knowledge-sandbox.py"), "--profile", "demo", *arguments, "--out", str(out)],
                   check=True, stdout=subprocess.DEVNULL, cwd=ROOT)
    return json.loads((out / "manifest.json").read_text(encoding="utf-8"))


class SliceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.out = Path(cls.tmp.name) / "slice"
        cls.manifest = generate(cls.out, "--slice")
        cls.questions = json.loads((cls.out / "questions.json").read_text(encoding="utf-8"))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_two_runs_give_the_same_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            generate(Path(other) / "slice", "--slice")
            self.assertEqual(tree_digest(self.out), tree_digest(Path(other) / "slice"))

    def test_every_new_format_is_in_the_slice_as_a_document_and_as_a_question(self):
        formats = self.manifest["documents_by_format"]
        for fmt in NEW_DOCUMENT_FORMATS:
            self.assertGreaterEqual(formats.get(fmt, 0), 2, fmt)
        asked = {q["feature"] for q in self.questions}
        for feature in READER_FEATURES.values():
            self.assertIn(feature, asked)

    def test_the_slice_holds_about_a_hundred_documents_forty_workbooks_and_thirty_questions(self):
        self.assertEqual(self.manifest["documents"], 101)  # the stream of 100, and the letter template
        self.assertTrue(35 <= self.manifest["workbooks"] <= 50, self.manifest["workbooks"])
        self.assertTrue(28 <= len(self.questions) <= 34, len(self.questions))
        self.assertEqual(set(self.manifest["workbooks_by_format"]), {"xlsx", "csv", "ods"})

    def test_every_expected_file_exists(self):
        for q in self.questions:
            for path in q["files"]["documents"]:
                self.assertTrue((self.out / "documents" / path).is_file(), path)
            for path in q["files"]["workbooks"]:
                self.assertTrue((self.out / "data" / path).is_file(), path)

    def test_the_question_ids_are_unique_and_a_follow_up_follows_its_question(self):
        ids = [q["id"] for q in self.questions]
        self.assertEqual(len(ids), len(set(ids)))
        for q in self.questions:
            if q["follow_up_of"]:
                self.assertLess(ids.index(q["follow_up_of"]), ids.index(q["id"]))

    def test_the_slice_is_small_enough_to_version(self):
        size = sum(p.stat().st_size for p in self.out.rglob("*") if p.is_file())
        self.assertLess(size, 8 * 1024 * 1024, size)


@unittest.skipIf(os.environ.get("ACAI_SKIP_SLOW") == "1", "the full library takes about twenty seconds")
class FullLibraryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.out = Path(cls.tmp.name) / "full"
        cls.manifest = generate(cls.out)
        cls.questions = json.loads((cls.out / "questions.json").read_text(encoding="utf-8"))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_the_documents_meet_their_floors(self):
        m = self.manifest
        self.assertGreaterEqual(m["readable_text_documents"], 1000)
        for fmt in NEW_DOCUMENT_FORMATS:
            self.assertGreaterEqual(m["documents_by_format"][fmt], 30, fmt)
        for fmt in ("pdf", "docx", "txt", "md", "scan-pdf", "jpg", "png"):
            self.assertGreater(m["documents_by_format"][fmt], 0, fmt)

    def test_the_workbooks_meet_their_floors(self):
        m = self.manifest
        self.assertGreaterEqual(m["workbooks"], 500)
        self.assertGreaterEqual(m["workbooks_by_format"]["ods"], 100)
        self.assertGreater(m["workbooks_by_format"]["xlsx"], 100)
        self.assertGreater(m["workbooks_by_format"]["csv"], 100)
        for trap in ("title_row", "merged_title", "formula_total", "empty_column", "notes_sheet", "text_dates", "swiss_numbers"):
            self.assertGreater(m["workbook_traps"][trap], 0, trap)

    def test_the_book_has_150_100_and_50_questions(self):
        blocks = Counter(q["block"] for q in self.questions)
        self.assertEqual((blocks["documents"], blocks["spreadsheets"], blocks["mixed"]), (150, 100, 50))

    def test_each_new_format_has_at_least_six_questions_and_ods_at_least_fifteen(self):
        per_format = Counter(q["format"] for q in self.questions if q["block"] == "documents")
        for fmt in NEW_DOCUMENT_FORMATS:
            self.assertGreaterEqual(per_format[fmt], 6, fmt)
        ods = sum(1 for q in self.questions if q["block"] == "spreadsheets" and q["format"] == "ods")
        self.assertGreaterEqual(ods, 15)

    def test_a_deck_question_has_a_slide_beyond_the_first(self):
        slides = [q["expect"].get("page") for q in self.questions if q["feature"] in ("D21", "D22")]
        self.assertTrue(any(page and page >= 2 for page in slides))

    def test_every_mixed_question_needs_a_document_and_a_workbook_or_is_a_routing_check(self):
        for q in self.questions:
            if q["block"] == "mixed" and q["feature"] in ("M01", "M02"):
                self.assertTrue(q["files"]["documents"] and q["files"]["workbooks"], q["id"])

    def test_every_feature_of_the_book_is_in_the_registry_and_nothing_is_forgotten(self):
        from large_test import features

        used = {q["feature"] for q in self.questions}
        self.assertTrue(used <= set(features.FEATURES))
        # Features covered by the format of the questions, not by a question of their own.
        dimension = {"T15", "T16", "T17"}
        self.assertEqual(set(features.FEATURES) - used - dimension, set())


class ScorerTests(unittest.TestCase):
    def judge(self, expect, replay, route=None, human=False):
        question = {"id": "X-1", "block": "documents", "feature": "D01", "format": "pdf", "expect": expect,
                    "route": route or ["global_retrieval"], "scored": {"auto": [], "human": human}}
        return score.Judge(question, replay).run(with_answer=True)

    def test_numbers_are_read_in_french_and_swiss_style(self):
        self.assertIn(1109.32, score.numbers_in("Le total est de CHF 1'109.32."))
        self.assertIn(1450.0, score.numbers_in("Montant : 1 450,00 €"))
        self.assertIn(6.8, score.numbers_in("HbA1c 6,8 %"))
        self.assertIn(1234.0, score.numbers_in("environ 1,234 unites") )

    def test_dates_are_read_in_words_and_in_figures(self):
        self.assertEqual(score.dates_in("le 24 octobre 2026 et le 1er mai 2027"), {"2026-10-24", "2027-05-01"})
        self.assertEqual(score.dates_in("rendez-vous le 08/05/2026 ou le 08.05.2026"), {"2026-05-08"})

    def test_a_value_a_date_and_words_are_judged(self):
        replay = {"route": "global_retrieval", "answer": "Le rendez-vous est le 24 octobre 2026, le total TTC est de CHF 1'109.32.", "sources": [{"path": "a.pdf", "page": 1}]}
        verdict = self.judge({"date": "2026-10-24", "value": 1109.32, "facts": [["total TTC"]], "sources_any": ["a.pdf"]}, replay)
        self.assertEqual(verdict["status"], "pass")
        wrong = self.judge({"date": "2026-10-25"}, replay)
        self.assertEqual(wrong["status"], "fail")

    def test_a_refusal_is_a_refusal_and_an_invented_date_is_not(self):
        refusal = {"refusal": True, "must_not_patterns": [r"\b\d{1,2} octobre 2026\b"]}
        ok = self.judge(refusal, {"route": "global_retrieval", "answer": "Les documents ne contiennent pas cette information.", "sources": []})
        self.assertEqual(ok["status"], "pass")
        invented = self.judge(refusal, {"route": "global_retrieval", "answer": "Ce n'est pas indiqué, mais le 12 octobre 2026 peut-être.", "sources": []})
        self.assertEqual(invented["status"], "fail")
        by_error = self.judge({"refusal": True}, {"route": "error", "error": "insufficient_evidence", "answer": "", "sources": []}, route=["error"])
        self.assertEqual(by_error["status"], "pass")

    def test_a_leak_is_counted(self):
        verdict = self.judge({"must_not": ["8 mai 2026"]}, {"route": "global_retrieval", "answer": "C'est le 8 mai 2026.", "sources": []})
        self.assertEqual(verdict["status"], "fail")

    def test_a_listing_needs_the_right_total_and_no_wrong_file(self):
        structured = {"kind": "document_search", "total": 2, "listed": [{"relativePath": "a.pdf"}, {"relativePath": "b.pdf"}], "others": []}
        replay = {"route": "set_search", "structured": structured, "structured_text": "a.pdf b.pdf", "answer": "", "sources": []}
        expect = {"listing": ["a.pdf", "b.pdf"], "listing_optional": [], "total": 2}
        self.assertEqual(self.judge(expect, replay, route=["set_search"])["status"], "pass")
        replay_wrong = dict(replay, structured=dict(structured, listed=[{"relativePath": "a.pdf"}, {"relativePath": "z.pdf"}]))
        self.assertEqual(self.judge(expect, replay_wrong, route=["set_search"])["status"], "fail")

    def test_a_nudge_is_recognised_by_its_reason_or_its_kind(self):
        replay = {"route": "tabular:nudge", "structured": {"kind": "nudge", "reason": "formula_cannot_be_verified"}, "structured_text": "", "answer": "", "sources": []}
        self.assertEqual(self.judge({"nudge": "formula_cannot_be_verified"}, replay, route=["tabular:nudge"])["status"], "pass")
        self.assertEqual(self.judge({"nudge": "which_workbook"}, replay, route=["tabular:nudge"])["status"], "fail")

    def test_a_page_must_be_the_one_expected(self):
        replay = {"route": "global_retrieval", "answer": "x", "sources": [{"path": "d.pptx", "page": 4}]}
        self.assertEqual(self.judge({"sources_any": ["d.pptx"], "page": 4}, replay)["status"], "pass")
        self.assertEqual(self.judge({"sources_any": ["d.pptx"], "page": 3}, replay)["status"], "fail")


if __name__ == "__main__":
    unittest.main()
