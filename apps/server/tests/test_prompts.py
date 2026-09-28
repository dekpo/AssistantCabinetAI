"""The base prompt's rules (docs/SELECTION-AND-MEMORY.md, "grounding outranks memory").

Two rounds of wording, both on 27 September 2026. The first added a no-documents path and, in
doing so, conditioned the "never invent a fact" rule on the model's own judgement of whether a
question "depends on the documents" - which let a model reason its way out of grounding and
hallucinate. The second round reverted that: the rule is unconditional again, and general-knowledge
answers are permitted only by the separate, narrower instruction Rust sends on the one path it has
already determined has no document (`conversation::NO_DOCUMENTS_INSTRUCTION` in the desktop crate) -
never as a choice the model makes for itself. These tests keep both rounds from drifting back.
"""

from __future__ import annotations

from assistant_cabinet_server.core.prompts import BASE_SYSTEM_PROMPT

#: Words that would assume a specific profession, medical above all - not "clinical", which the
#: prompt uses once, deliberately, as one of three parallel examples (clinical, legal, financial)
#: precisely to demonstrate it is not assuming medicine. This product is meant to read the same
#: for a doctor, a lawyer, a notary or an accountant; none of that is its business to assume, so
#: none of it belongs, unprompted, in a string sent to the model.
PROFESSION_MARKERS = ["patient", "doctor", "practitioner", "practice", "gp"]


def test_never_inventing_a_fact_is_unconditional() -> None:
    assert (
        "You never invent a fact. When the information you were given does not carry the"
        in BASE_SYSTEM_PROMPT
    )
    # The conditional form is the regression this test exists to catch: it let the model decide
    # for itself when honesty was optional.
    assert "depends on" not in BASE_SYSTEM_PROMPT


def test_the_rules_are_kept_out_of_the_answer() -> None:
    assert "they are not part of them. Never quote, list or mention them" in BASE_SYSTEM_PROMPT


def test_the_safety_rules_are_unchanged() -> None:
    assert "never give professional advice of any kind" in BASE_SYSTEM_PROMPT
    assert "never invent a fact" in BASE_SYSTEM_PROMPT
    assert "never emit a tool call" in BASE_SYSTEM_PROMPT


def test_the_prompt_never_names_a_profession() -> None:
    lower = BASE_SYSTEM_PROMPT.lower()
    for word in PROFESSION_MARKERS:
        assert word not in lower, f"{word!r} found in BASE_SYSTEM_PROMPT"


def test_the_prompt_speaks_of_the_user_rather_than_an_abstract_practice() -> None:
    assert "the user" in BASE_SYSTEM_PROMPT
