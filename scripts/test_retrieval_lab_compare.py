"""Tests for scripts/retrieval_lab_compare.py on a tiny invented exam. Run from scripts/: python -m unittest test_retrieval_lab_compare"""

import unittest

import retrieval_lab_compare as compare


def question(qid: str, category: str, relevant: list[str]) -> dict:
    return {
        "id": qid,
        "category": category,
        "text": f"question {qid}",
        "relevant_readable": relevant,
        "expect": {"contains": []},
    }


def row(qid: str, results: list[dict]) -> dict:
    return {"id": qid, "channel": "pipeline", "route": "retrieval", "ms": 1.0, "results": results}


def passage(path: str, section: str | None = None, page: int = 1) -> dict:
    found = {"path": path, "page": page, "chunk_id": f"{path}#{page}", "score": 0.0, "fact": False}
    if section is not None:
        found["section_label"] = section
    return found


class Compare(unittest.TestCase):
    questions = {
        "Q1": question("Q1", "person", ["a"]),
        "Q2": question("Q2", "identifier", ["b"]),
    }

    def test_a_family_that_loses_recall_is_flagged_and_the_question_is_named(self):
        base = {"Q1": row("Q1", [passage("a")]), "Q2": row("Q2", [passage("b")])}
        other = {"Q1": row("Q1", [passage("a")]), "Q2": row("Q2", [passage("x")])}
        lines, worse = compare.compare_channel(self.questions, base, other, "category", "pipeline")
        text = "\n".join(lines)
        self.assertIn("WORSE  identifier", text)
        self.assertNotIn("WORSE  person", text)
        self.assertEqual([item[0] for item in worse if item[1] == "recall@10"], ["Q2"])
        self.assertIn("question Q2", text)

    def test_a_family_that_gains_is_better_and_nothing_is_flagged(self):
        base = {"Q1": row("Q1", [passage("x")]), "Q2": row("Q2", [passage("b")])}
        other = {"Q1": row("Q1", [passage("a")]), "Q2": row("Q2", [passage("b")])}
        lines, worse = compare.compare_channel(self.questions, base, other, "category", "pipeline")
        self.assertIn("better person", "\n".join(lines))
        self.assertEqual(worse, [])

    def test_the_spread_counts_distinct_files_and_sections_in_the_first_six_passages(self):
        rows = {
            "Q1": row("Q1", [passage("a", "one")] * 3 + [passage("a", "two")]),
            "Q2": row("Q2", [passage("b", "one"), passage("c", "one")]),
        }
        self.assertAlmostEqual(compare.spread(rows, self.questions), (2 + 2) / 2)

    def test_the_spread_line_shows_when_the_places_are_spread_wider(self):
        base = {"Q1": row("Q1", [passage("a", "one")] * 4), "Q2": row("Q2", [passage("b", "one")] * 4)}
        other = {"Q1": row("Q1", [passage("a", "one"), passage("a", "two"), passage("c", "one"), passage("a", "one")]),
                 "Q2": row("Q2", [passage("b", "one")] * 4)}
        lines, _ = compare.compare_channel(self.questions, base, other, "category", "pipeline")
        self.assertIn("places spread (distinct file+section in the first 6): 1.00 -> 2.00", "\n".join(lines))


if __name__ == "__main__":
    unittest.main()
