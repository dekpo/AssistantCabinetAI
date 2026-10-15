"""Tests for scripts/retrieval_lab_refusal.py on a tiny invented exam. Run from scripts/: python -m unittest test_retrieval_lab_refusal"""

import unittest

import retrieval_lab_refusal as refusal


def question(qid: str, relevant: list[str], category: str = "person") -> dict:
    return {"id": qid, "category": category, "text": f"question {qid}", "relevant_readable": relevant}


class Judge(unittest.TestCase):
    questions = {
        "Q1": question("Q1", ["a"]),
        "Q2": question("Q2", ["b"]),
        "Q3": question("Q3", [], "unknown"),
        "Q4": question("Q4", [], "unknown"),
        "Q5": question("Q5", [], "data"),
    }

    def test_a_refused_answerable_question_is_a_false_refusal_and_a_refused_unanswerable_one_a_true_refusal(self):
        rows = {
            "Q1": {"refused": False},
            "Q2": {"refused": True},
            "Q3": {"refused": True},
            "Q4": {"refused": False},
        }
        verdict = refusal.judge(self.questions, rows)
        self.assertEqual(verdict["answerable"], 2)
        self.assertEqual([qid for qid, _ in verdict["answerable_refused"]], ["Q2"])
        self.assertEqual(verdict["unanswerable"], 2)
        self.assertEqual(verdict["unanswerable_refused"], ["Q3"])

    def test_a_question_about_the_tables_is_out_of_scope(self):
        verdict = refusal.judge(self.questions, {"Q5": {"refused": True}})
        self.assertEqual((verdict["answerable"], verdict["unanswerable"]), (0, 0))


if __name__ == "__main__":
    unittest.main()
