"""The output language comes from the request, not from the prompt or the code.

These tests are the reason no French is baked into a code path: the same English body plus a
rendered directive produces a French answer or an English one.
"""

from __future__ import annotations

from fastapi.testclient import TestClient

from assistant_cabinet_server.core.prompts import BASE_SYSTEM_PROMPT

from .conftest import FakeProvider

QUESTION = {"messages": [{"role": "user", "content": "Bonjour"}]}


def test_the_directive_names_the_requested_language(
    client: TestClient, provider: FakeProvider
) -> None:
    client.post("/v1/chat/completions", json={**QUESTION, "output_locale": "fr-FR"})

    prompt = provider.last_system_prompt
    assert prompt.startswith(BASE_SYSTEM_PROMPT)
    assert "write every part of your answer in French (fr-FR)" in prompt
    assert "write extrait" in prompt


def test_the_same_body_serves_another_language(client: TestClient, provider: FakeProvider) -> None:
    client.post("/v1/chat/completions", json={**QUESTION, "output_locale": "en-US"})

    prompt = provider.last_system_prompt
    assert prompt.startswith(BASE_SYSTEM_PROMPT)
    assert "write every part of your answer in English (en-US)" in prompt
    assert "write excerpt" in prompt


def test_a_missing_locale_falls_back_to_the_default_and_never_to_english(
    client: TestClient, provider: FakeProvider
) -> None:
    client.post("/v1/chat/completions", json=QUESTION)

    prompt = provider.last_system_prompt
    assert "(fr-FR)" in prompt
    assert "in English" not in prompt


def test_a_language_subtag_matches_a_shipped_pack(
    client: TestClient, provider: FakeProvider
) -> None:
    client.post("/v1/chat/completions", json={**QUESTION, "output_locale": "fr"})

    assert "(fr-FR)" in provider.last_system_prompt


def test_an_unsupported_locale_is_refused_rather_than_answered_in_english(
    client: TestClient, provider: FakeProvider
) -> None:
    response = client.post("/v1/chat/completions", json={**QUESTION, "output_locale": "de-DE"})

    assert response.status_code == 400
    error = response.json()["error"]
    assert error["code"] == "output_locale_not_supported"
    assert error["data"]["requested"] == "de-DE"
    assert "fr-FR" in error["data"]["supported"]
    assert provider.requests == []


def test_the_system_prompt_forbids_a_model_written_disclaimer(
    client: TestClient, provider: FakeProvider
) -> None:
    # The client appends the notice from its own catalogue: asking a model to reproduce a fixed
    # legal sentence is unreliable and it would hardcode one language into an English prompt.
    client.post("/v1/chat/completions", json=QUESTION)

    assert "Do not append any disclaimer" in provider.last_system_prompt
