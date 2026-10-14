"""Tests for scripts/file_profile_score.py on a tiny invented library. Run from scripts/: python -m unittest test_file_profile_score"""

import unittest
from pathlib import Path

import file_profile_score as fp

TRUTH = {
    "courriers/2026-02-12_courrier-orl-dupont-jean.pdf": {
        "domain": "documents", "business_type": "letter", "document_date": "2026-02-12"},
    "courriers/2026-03-01_courrier-orl-martin-marie.pdf": {
        "domain": "documents", "business_type": "letter", "document_date": "2026-03-01"},
    "notes/note-interne.txt": {"domain": "documents", "business_type": "other", "document_date": None},
    "rendez-vous-2026.xlsx": {"domain": "data", "format": "xlsx", "sheets": ["Septembre"]},
}


class NameWords(unittest.TestCase):
    def test_a_date_prefix_and_the_extension_are_not_words(self):
        self.assertEqual(
            fp.name_words("courriers/2026-02-12_courrier-orl-dupont-jean.pdf"),
            ["courrier", "orl", "dupont", "jean", "courriers"],
        )

    def test_accents_and_capitals_are_folded(self):
        self.assertEqual(fp.name_words("Ordonnance-Esaie_Hopital.pdf")[:3], ["ordonnance", "esaie", "hopital"])


class NameQueries(unittest.TestCase):
    def queries(self):
        return {(q["kind"], q["query"]): set(q["expected"]) for q in fp.name_queries(TRUTH)}

    def test_the_full_name_and_the_stem_find_their_own_file(self):
        found = self.queries()
        self.assertEqual(found[("full_name", "note-interne.txt")], {"notes/note-interne.txt"})
        self.assertEqual(found[("stem", "note-interne")], {"notes/note-interne.txt"})

    def test_a_query_that_two_files_satisfy_expects_both(self):
        found = self.queries()
        self.assertEqual(found[("word_prefix", "cou")], {
            "courriers/2026-02-12_courrier-orl-dupont-jean.pdf",
            "courriers/2026-03-01_courrier-orl-martin-marie.pdf",
        })

    def test_the_name_without_its_date_prefix_is_a_query(self):
        self.assertIn(("no_date_prefix", "courrier-orl-dupont-jean"), self.queries())

    def test_two_words_in_the_other_order_are_a_query(self):
        self.assertIn(("two_words_reversed", "orl courrier"), self.queries())


class Score(unittest.TestCase):
    def test_a_wrong_value_lowers_precision_and_an_empty_one_lowers_the_fill_rate(self):
        profiles = [
            {"path": "courriers/2026-02-12_courrier-orl-dupont-jean.pdf", "business_type": "letter", "document_date": "2026-02-12"},
            {"path": "courriers/2026-03-01_courrier-orl-martin-marie.pdf", "business_type": "invoice", "document_date": None},
        ]
        report = fp.score(profiles, TRUTH)["business_type"]
        self.assertEqual((report["known"], report["filled"], report["correct"]), (2, 2, 1))
        self.assertEqual(report["precision"], 0.5)
        date = fp.score(profiles, TRUTH)["document_date"]
        self.assertEqual((date["filled"], date["fill_rate"]), (1, 0.5))

    def test_other_is_right_when_left_empty(self):
        report = fp.score([{"path": "notes/note-interne.txt", "business_type": None}], TRUTH)["business_type"]
        self.assertEqual(report["filled"], 0)


class ByFormatAndRoles(unittest.TestCase):
    TRUTH = {
        "a.pdf": {"domain": "documents", "format": "pdf", "business_type": "letter", "document_date": "2026-01-01",
                  "about_forms": [["marie martin", "martin marie"]]},
        "b.docx": {"domain": "documents", "format": "docx", "business_type": "invoice", "document_date": "2026-01-02",
                   "about_forms": [["jean dupont"]]},
        "c.odt": {"domain": "documents", "format": "odt", "business_type": "letter", "document_date": "2026-01-03",
                  "about_forms": []},
    }

    def test_the_figures_are_given_per_format_and_for_the_analysed_files_only(self):
        profiles = [
            {"domain": "documents", "path": "a.pdf", "business_type": "letter", "document_date": "2026-01-01"},
            {"domain": "documents", "path": "b.docx", "business_type": "letter", "document_date": None},
            {"domain": "data", "path": "c.odt", "business_type": "letter", "document_date": "2026-01-03"},
        ]
        report = fp.score(profiles, self.TRUTH)["business_type"]
        self.assertEqual((report["known"], report["filled"], report["correct"]), (3, 2, 1))
        self.assertEqual((report["analysed"]["known"], report["analysed"]["fill_rate"]), (2, 1.0))
        self.assertEqual(report["by_format"]["odt"]["filled"], 0, "a data-folder entry is not a document")
        self.assertEqual(report["by_format"]["pdf"]["precision"], 1.0)
        self.assertEqual(report["by_format"]["docx"]["precision"], 0.0)

    def test_a_subject_is_right_when_it_is_one_of_the_ways_the_person_is_written(self):
        profiles = [
            {"domain": "documents", "path": "a.pdf", "roles": [
                {"role": "subject", "surface": "Marie Martin, nee en 1990", "canonical": None}]},
            {"domain": "documents", "path": "b.docx", "roles": [
                {"role": "subject", "surface": "Someone Else", "canonical": "Paul Roy"},
                {"role": "author", "surface": "Jean Dupont", "canonical": None}]},
        ]
        report = fp.score_roles(profiles, self.TRUTH)
        self.assertEqual((report["known"], report["filled"], report["correct"]), (2, 2, 1))
        self.assertEqual(report["precision"], 0.5)
        self.assertIsNone(fp.score_roles([], self.TRUTH)["fill_rate"], "nothing analysed, nothing known")


class RealSandbox(unittest.TestCase):
    def test_the_sandbox_truth_loads_and_every_file_has_a_name_query(self):
        sandbox = Path(__file__).resolve().parent.parent / "fixtures" / "knowledge-sandbox"
        truth = fp.truth_from_sandbox(sandbox)
        covered = {path for q in fp.name_queries(truth) if q["kind"] == "full_name" for path in q["expected"]}
        self.assertEqual(covered, set(truth))
        self.assertEqual(truth["rendez-vous-2026.xlsx"]["sheets"], ["Septembre", "Octobre"])


if __name__ == "__main__":
    unittest.main()
