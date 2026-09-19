"""Aliases resolve in configuration, and a weight name never reaches a client."""

from __future__ import annotations

from fastapi.testclient import TestClient

from .conftest import CHAT_ALIAS, CONFIGURED_ALIASES, RECORDED_MODEL, FakeProvider


def test_an_alias_resolves_to_the_configured_model(
    client: TestClient, provider: FakeProvider
) -> None:
    response = client.post(
        "/v1/chat/completions",
        json={"model": CHAT_ALIAS, "messages": [{"role": "user", "content": "Bonjour"}]},
    )

    assert response.status_code == 200
    assert provider.requests[-1].model == RECORDED_MODEL
    # The answer names the alias the client asked for, not what it resolved to.
    assert response.json()["model"] == CHAT_ALIAS
    assert RECORDED_MODEL not in response.text


def test_a_request_without_a_model_uses_the_default_alias(
    client: TestClient, provider: FakeProvider
) -> None:
    response = client.post(
        "/v1/chat/completions", json={"messages": [{"role": "user", "content": "Bonjour"}]}
    )

    assert response.status_code == 200
    assert response.json()["model"] == CHAT_ALIAS
    assert provider.requests[-1].model == RECORDED_MODEL


def test_an_unknown_alias_is_refused_with_a_code(client: TestClient) -> None:
    response = client.post(
        "/v1/chat/completions",
        json={"model": "gpt-4o", "messages": [{"role": "user", "content": "Bonjour"}]},
    )

    assert response.status_code == 400
    error = response.json()["error"]
    assert error["code"] == "model_alias_not_allowed"
    assert error["data"] == {"requested": "gpt-4o", "allowed": CONFIGURED_ALIASES}


def test_the_model_list_exposes_aliases_only(client: TestClient) -> None:
    response = client.get("/v1/models")

    body = response.json()
    assert [card["id"] for card in body["data"]] == CONFIGURED_ALIASES
    assert RECORDED_MODEL not in response.text
