"""Streaming, and what happens when the runtime does not answer."""

from __future__ import annotations

import json

from fastapi.testclient import TestClient

from assistant_cabinet_server.core.errors import ErrorCode, GatewayError
from assistant_cabinet_server.main import create_app

from .conftest import RECORDED_MODEL, FakeProvider, build_settings

QUESTION = {"messages": [{"role": "user", "content": "Bonjour"}]}


def test_a_stream_carries_deltas_and_ends_with_done(client: TestClient) -> None:
    with client.stream(
        "POST", "/v1/chat/completions", json={**QUESTION, "stream": True}
    ) as response:
        assert response.status_code == 200
        assert response.headers["content-type"].startswith("text/event-stream")
        events = [
            line.removeprefix("data: ")
            for line in response.iter_lines()
            if line.startswith("data: ")
        ]

    assert events[-1] == "[DONE]"
    payloads = [json.loads(event) for event in events[:-1]]
    assert payloads[0]["choices"][0]["delta"]["role"] == "assistant"
    assert payloads[-1]["choices"][0]["finish_reason"] == "stop"
    answer = "".join(payload["choices"][0]["delta"].get("content") or "" for payload in payloads)
    assert answer.strip() == "Bonjour."
    assert all(payload["model"] == "cabinet-chat" for payload in payloads)
    assert RECORDED_MODEL not in " ".join(events)


def test_a_streamed_request_is_registered_once(client: TestClient) -> None:
    with client.stream("POST", "/v1/chat/completions", json={**QUESTION, "stream": True}) as r:
        r.read()

    entries = client.app.state.register.entries  # type: ignore[attr-defined]
    assert len(entries) == 1
    assert entries[0].completion_chars > 0
    assert entries[0].outcome == "completed"


def test_an_unreachable_runtime_answers_with_a_code_not_a_traceback() -> None:
    failure = GatewayError(
        ErrorCode.provider_unreachable, status_code=503, data={"provider": "fake"}
    )
    app = create_app(settings=build_settings(), provider=FakeProvider(failure=failure))
    with TestClient(app) as client:
        response = client.post("/v1/chat/completions", json={**QUESTION, "stream": True})

    assert response.status_code == 503
    assert response.json()["error"]["code"] == "provider_unreachable"
    assert "Traceback" not in response.text


def test_an_oversized_request_is_refused_before_it_reaches_the_runtime() -> None:
    provider = FakeProvider()
    app = create_app(settings=build_settings(MAX_CONTEXT_CHARS=500), provider=provider)
    with TestClient(app) as client:
        response = client.post(
            "/v1/chat/completions",
            json={"messages": [{"role": "user", "content": "x" * 900}]},
        )

    assert response.status_code == 413
    error = response.json()["error"]
    assert error["code"] == "context_too_large"
    assert error["data"]["limit"] == 500
    assert provider.requests == []
