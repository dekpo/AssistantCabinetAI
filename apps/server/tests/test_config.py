"""Configuration read from the environment, the way the container reads it."""

from __future__ import annotations

import pytest

from assistant_cabinet_server.core.config import Settings


def test_aliases_are_read_from_the_pair_form_compose_uses(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("MODEL_ALIASES", "cabinet-chat=mistral,cabinet-rapide=qwen3:8b")

    settings = Settings()  # type: ignore[call-arg]

    assert settings.model_aliases == {"cabinet-chat": "mistral", "cabinet-rapide": "qwen3:8b"}
    assert settings.allowed_aliases == ["cabinet-chat", "cabinet-rapide"]


def test_chat_aliases_excludes_the_embedding_alias(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv(
        "MODEL_ALIASES",
        "cabinet-chat=mistral,cabinet-rapide=qwen3:8b,cabinet-embed=nomic-embed-text",
    )
    monkeypatch.setenv("DEFAULT_EMBEDDING_ALIAS", "cabinet-embed")

    settings = Settings()  # type: ignore[call-arg]

    assert settings.allowed_aliases == ["cabinet-chat", "cabinet-embed", "cabinet-rapide"]
    # A human picks a chat profile in Settings; the embedding alias is a backend-only
    # concern (docs/RETRIEVAL.md) and must never show up as a choice there.
    assert settings.chat_aliases == ["cabinet-chat", "cabinet-rapide"]


def test_aliases_are_also_accepted_as_json(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MODEL_ALIASES", '{"cabinet-chat": "mistral"}')

    settings = Settings()  # type: ignore[call-arg]

    assert settings.model_aliases == {"cabinet-chat": "mistral"}


def test_a_gateway_without_configuration_serves_nothing(monkeypatch: pytest.MonkeyPatch) -> None:
    # Serving whatever model happens to be installed would defeat the allow-list.
    monkeypatch.delenv("MODEL_ALIASES", raising=False)

    settings = Settings()  # type: ignore[call-arg]

    assert settings.model_aliases == {}
    assert settings.default_output_locale == "fr-FR"


def test_a_malformed_alias_entry_is_refused(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MODEL_ALIASES", "cabinet-chat")

    with pytest.raises(ValueError):
        Settings()  # type: ignore[call-arg]
