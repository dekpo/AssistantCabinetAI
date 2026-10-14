"""Tests for scripts/retrieval_lab_report.py. Run from scripts/: python -m unittest test_retrieval_lab_report"""

import unittest

import retrieval_lab_report as report


def passage(path, fact=False):
    return {"path": path, "page": 1, "chunk_id": path + "#1", "score": 0.0, "fact": fact}


QUESTION = {"id": "Q1", "category": "paraphrase", "text": "Quand ?", "relevant_readable": ["d/a.pdf"],
            "expect": {"contains": ["12 mai"]}, "fact_pages": [1]}


class Cells(unittest.TestCase):
    def test_a_fact_in_the_second_cited_passage_reads_ok_2(self):
        row = {"results": [passage("d/x.pdf"), passage("d/a.pdf", fact=True)]}
        self.assertEqual(report.cell(QUESTION, row, None), "ok 2")

    def test_a_fact_only_in_the_deep_ranking_reads_deep_with_its_rank(self):
        row = {"results": [passage("d/x.pdf")]}
        deep = {"results": [passage("d/x.pdf")] * 8 + [passage("d/a.pdf", fact=True)]}
        self.assertEqual(report.cell(QUESTION, row, deep), "no (deep 9)")

    def test_a_fact_in_a_passage_of_another_file_does_not_count(self):
        row = {"results": [passage("d/x.pdf", fact=True)]}
        self.assertEqual(report.cell(QUESTION, row, None), "no")

    def test_an_absent_question_reads_the_number_of_passages_still_cited(self):
        question = dict(QUESTION, relevant_readable=[], expect={"contains": []})
        self.assertEqual(report.cell(question, {"results": [passage("d/x.pdf")] * 6}, None), "6 passages")

    def test_a_question_without_a_fact_reads_the_relevant_files_cited(self):
        question = dict(QUESTION, relevant_readable=["d/a.pdf", "d/b.pdf"], expect={"contains": []})
        self.assertEqual(report.cell(question, {"results": [passage("d/a.pdf")]}, None), "files 1/2")

    def test_the_expected_cell_names_the_file_the_fact_and_the_page(self):
        self.assertEqual(report.expected(QUESTION), "a.pdf - `12 mai` (p. 1)")


if __name__ == "__main__":
    unittest.main()
