"""The base prompt's rules, as decided on 27 September 2026 (docs/SELECTION-AND-MEMORY.md).

Two things small models got wrong: repeating "I do not have the information" to questions that
never needed the practice's documents, and ending answers by reciting their rules. The wording
below is what addresses them; these tests keep it from drifting back.
"""

from __future__ import annotations

from assistant_cabinet_server.core.prompts import BASE_SYSTEM_PROMPT


def test_saying_so_is_about_questions_that_depend_on_the_documents() -> None:
    assert "When a question\n  depends on the practice's documents" in BASE_SYSTEM_PROMPT
    # The unconditional form is what made a model refuse general questions.
    assert "When the information you were given does not carry the answer" not in BASE_SYSTEM_PROMPT


def test_the_rules_are_kept_out_of_the_answer() -> None:
    assert "they are not part of them. Never quote, list or mention them" in BASE_SYSTEM_PROMPT


def test_the_safety_rules_are_unchanged() -> None:
    assert "never give clinical, diagnostic or prescribing advice" in BASE_SYSTEM_PROMPT
    assert "never invent a fact" in BASE_SYSTEM_PROMPT
    assert "never emit a tool call" in BASE_SYSTEM_PROMPT
