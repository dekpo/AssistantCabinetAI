"""Every request carries a context window chosen on purpose, and `/health` publishes it.

Before 27 September 2026 the window was left to the runtime's default, which silently dropped the
oldest messages of a conversation. The window is now configuration, capped at what the model
supports, and the desktop app reads it to decide how much of the conversation fits
(docs/SELECTION-AND-MEMORY.md).
"""

from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from assistant_cabinet_server.core.config import Settings
from assistant_cabinet_server.main import create_app

from .conftest import (
    CHAT_ALIAS,
    EMBED_ALIAS,
    FAST_ALIAS,
    RECORDED_MODEL,
    FakeProvider,
    build_settings,
)

QUESTION = {"model": CHAT_ALIAS, "messages": [{"role": "user", "content": "Bonjour"}]}


def ask(provider: FakeProvider, alias: str = CHAT_ALIAS, **settings: object) -> int | None:
    app = create_app(settings=build_settings(**settings), provider=provider)
    with TestClient(app) as client:
        response = client.post("/v1/chat/completions", json={**QUESTION, "model": alias})
    assert response.status_code == 200
    return provider.requests[-1].context_window


def test_every_request_carries_the_default_window() -> None:
    assert ask(FakeProvider()) == 8_192


def test_an_alias_can_have_its_own_window() -> None:
    provider = FakeProvider()

    assert ask(provider, FAST_ALIAS, MODEL_CONTEXT_WINDOWS=f"{FAST_ALIAS}=16384") == 16_384
    assert ask(provider, CHAT_ALIAS, MODEL_CONTEXT_WINDOWS=f"{FAST_ALIAS}=16384") == 8_192


def test_the_window_never_exceeds_what_the_model_supports() -> None:
    provider = FakeProvider(context_limit=4_096)

    assert ask(provider, DEFAULT_CONTEXT_WINDOW="32768") == 4_096
    assert provider.context_limit_requests[-1] == RECORDED_MODEL


def test_health_publishes_each_chat_alias_window_and_the_answer_reserve() -> None:
    app = create_app(
        settings=build_settings(MODEL_CONTEXT_WINDOWS=f"{FAST_ALIAS}=4096"),
        provider=FakeProvider(),
    )
    with TestClient(app) as client:
        body = client.get("/health").json()

    assert body["context_windows"] == {CHAT_ALIAS: 8_192, FAST_ALIAS: 4_096}
    # The embedding alias answers nothing, so it has no window to publish.
    assert EMBED_ALIAS not in body["context_windows"]
    assert body["max_output_tokens"] == 2_048


def test_health_does_not_ask_a_runtime_that_is_down() -> None:
    provider = FakeProvider(reachable=False, context_limit=1_024)
    app = create_app(settings=build_settings(), provider=provider)
    with TestClient(app) as client:
        body = client.get("/health").json()

    assert provider.context_limit_requests == []
    assert body["context_windows"] == {CHAT_ALIAS: 8_192, FAST_ALIAS: 8_192}


def test_windows_are_read_from_the_pair_form_or_json(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MODEL_CONTEXT_WINDOWS", "cabinet-chat=16384,cabinet-rapide=4096")
    assert Settings().model_context_windows == {  # type: ignore[call-arg]
        "cabinet-chat": 16_384,
        "cabinet-rapide": 4_096,
    }

    monkeypatch.setenv("MODEL_CONTEXT_WINDOWS", '{"cabinet-chat": 2048}')
    assert Settings().context_window_for("cabinet-chat") == 2_048  # type: ignore[call-arg]
    assert Settings().context_window_for("anything-else") == 8_192  # type: ignore[call-arg]


def test_a_malformed_window_entry_is_refused(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("MODEL_CONTEXT_WINDOWS", "cabinet-chat=large")

    with pytest.raises(ValueError):
        Settings()  # type: ignore[call-arg]
