"""Tests for scripts/retrieval_lab_score.py on a tiny invented exam. Run from scripts/: python -m unittest test_retrieval_lab_score"""

import json
import tempfile
import unittest
from pathlib import Path

import retrieval_lab_score as score


def passage(path: str, fact: bool = False) -> dict:
    return {"path": path, "page": 1, "chunk_id": path + "#1", "score": 0.0, "fact": fact}


class ScoreQuestion(unittest.TestCase):
    def test_a_relevant_file_at_rank_two_gives_half_the_reciprocal_rank(self):
        figures = score.score_question([passage("x"), passage("a")], {"a"}, False)
        self.assertEqual(figures["mrr"], 0.5)
        self.assertEqual(figures["recall@5"], 1.0)
        self.assertEqual(figures["wrong_files"], 1)

    def test_a_file_counts_once_however_many_of_its_passages_come_back(self):
        figures = score.score_question([passage("a"), passage("a"), passage("a")], {"a", "b"}, False)
        self.assertEqual(figures["hits"], 1)
        self.assertEqual(figures["recall@5"], 0.5)

    def test_recall_at_five_ignores_a_file_that_comes_after_the_cutoff(self):
        results = [passage(f"x{i}") for i in range(5)] + [passage("a")]
        figures = score.score_question(results, {"a"}, False)
        self.assertEqual(figures["recall@5"], 0.0)
        self.assertEqual(figures["recall@10"], 1.0)
        self.assertEqual(figures["recall@30"], 1.0)

    def test_a_perfect_ranking_has_an_ndcg_of_one(self):
        figures = score.score_question([passage("a"), passage("b")], {"a", "b"}, False)
        self.assertAlmostEqual(figures["ndcg@10"], 1.0)

    def test_nothing_found_scores_zero_everywhere(self):
        figures = score.score_question([passage("x")], {"a"}, False)
        self.assertEqual((figures["mrr"], figures["recall@30"], figures["ndcg@10"]), (0.0, 0.0, 0.0))

    def test_the_planted_fact_is_looked_for_in_the_first_k_passages_only(self):
        results = [passage(f"x{i}") for i in range(6)] + [passage("a", fact=True)]
        figures = score.score_question(results, {"a"}, True)
        self.assertEqual(figures["fact@5"], 0.0)
        self.assertEqual(figures["fact@10"], 1.0)


    def test_a_fact_in_a_passage_of_another_file_does_not_count(self):
        results = [passage("other", fact=True), passage("a", fact=False)]
        figures = score.score_question(results, {"a"}, True)
        self.assertEqual(figures["fact@5"], 0.0)


class PlantedFacts(unittest.TestCase):
    def labelled(self, path: str, label: str, fact: bool = True) -> dict:
        return {**passage(path, fact=fact), "section_label": label}

    def test_a_fact_counts_for_its_section_only_when_the_passage_names_that_section(self):
        results = [self.labelled("a", "2.1 Hygiene"), self.labelled("a", "9.9 Other")]
        right = score.score_question(results, {"a"}, True, None, ["2.1 Hygiene"])
        self.assertEqual(right["section@10"], 1.0)
        wrong = score.score_question(results[1:], {"a"}, True, None, ["2.1 Hygiene"])
        self.assertEqual(wrong["section@10"], 0.0)

    def test_the_section_is_not_scored_when_the_passages_carry_no_label(self):
        figures = score.score_question([passage("a", fact=True)], {"a"}, True, None, ["2.1 Hygiene"])
        self.assertNotIn("section@10", figures)

    def test_the_figures_are_grouped_by_kind_of_planted_fact(self):
        questions = {
            "Q1": {"id": "Q1", "category": "planted", "plant": "table_split", "relevant_readable": ["a"],
                   "relevant_documents": ["a"], "expect": {"contains": ["x"]}, "text": "t1", "fact_pages": [3]},
            "Q2": {"id": "Q2", "category": "planted", "plant": "prose", "relevant_readable": ["b"],
                   "relevant_documents": ["b"], "expect": {"contains": ["y"]}, "text": "t2"},
            "Q3": {"id": "Q3", "category": "absent", "plant": "absent", "relevant_readable": [],
                   "relevant_documents": [], "expect": {"contains": []}, "text": "t3"},
        }
        rows = {
            "Q1": {"id": "Q1", "channel": "c", "route": "r", "ms": 1.0, "results": [passage("a", fact=False)]},
            "Q2": {"id": "Q2", "channel": "c", "route": "r", "ms": 2.0, "results": [passage("b", fact=True)]},
            "Q3": {"id": "Q3", "channel": "c", "route": "r", "ms": 3.0, "results": [passage("z"), passage("y")]},
        }
        summary = score.summarise_channel(questions, rows)
        groups = score.by_plant(questions, summary, rows)
        self.assertEqual(groups["table_split"]["fact@10"], 0.0)
        self.assertEqual(groups["prose"]["fact@10"], 1.0)
        self.assertEqual(groups["absent"]["passages_returned"], 2.0)
        self.assertEqual(groups["absent"]["n"], 1)


class WholeRun(unittest.TestCase):
    def setUp(self):
        self.folder = Path(tempfile.mkdtemp())
        self.questions = {
            "Q1": {"id": "Q1", "category": "term", "relevant_readable": ["a", "b"], "relevant_documents": ["a", "b"],
                   "expect": {"contains": []}, "text": "Quels documents parlent de Nyon ?"},
            "Q2": {"id": "Q2", "category": "unknown", "relevant_readable": [], "relevant_documents": [],
                   "expect": {"contains": []}, "text": "Horaires ?"},
        }
        rows = [
            {"id": "Q1", "channel": "pipeline", "route": "global", "ms": 4.0, "results": [passage("a")]},
            {"id": "Q1", "channel": "term_all", "route": "global", "ms": 1.0, "results": [passage("a"), passage("b"), passage("z")]},
            {"id": "Q2", "channel": "pipeline", "route": "global", "ms": 6.0, "results": [passage("a"), passage("b")]},
        ]
        (self.folder / "run.jsonl").write_text("\n".join(json.dumps(r) for r in rows), encoding="utf-8")

    def test_a_question_the_documents_cannot_answer_is_reported_by_passages_returned(self):
        run = score.load_run(self.folder)
        summary = score.summarise_channel(self.questions, run["pipeline"])
        self.assertEqual(summary["questions"], 1)
        self.assertEqual(summary["unanswerable"], 1)
        self.assertEqual(summary["unanswerable_passages"], 2.0)

    def test_the_set_table_counts_the_files_the_word_search_found_and_the_wrong_ones(self):
        run = score.load_run(self.folder)
        table = score.set_sentences(self.questions, run)
        self.assertIn("| Q1 | Quels documents parlent de Nyon ? | 2 | 1 | 0 | 2 | 1 |", table)

    def test_the_command_line_runs_end_to_end(self):
        questions = self.folder / "questions.json"
        questions.write_text(json.dumps(list(self.questions.values())), encoding="utf-8")
        self.assertEqual(score.main([str(self.folder), "--questions", str(questions), "--categories", "--detail", "Q1"]), 0)


if __name__ == "__main__":
    unittest.main()
