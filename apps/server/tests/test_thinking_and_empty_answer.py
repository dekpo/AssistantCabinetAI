"""Hidden reasoning, and a generation that ends with nothing to show.

Measured on 9 October 2026 (`docs/test-reports/small-model-comparison-1/`, section 10): some models
write a reasoning before the answer, the gateway throws it away, and the tokens count against the
output budget. A model that spends the whole budget thinking returned a blank message with no
error (KBD-08). These tests drive the real provider over an in-memory transport, so nothing here
needs Ollama.
"""

from __future__ import annotations

import json
import logging
from collections.abc import Callable, Iterator

import httpx
import pytest
from fastapi.testclient import TestClient
from pydantic import ValidationError

from assistant_cabinet_server.core.config import Settings
from assistant_cabinet_server.core.no_store import PACKAGE_LOGGER_NAME
from assistant_cabinet_server.main import create_app
from assistant_cabinet_server.providers import GenerationRequest, Message
from assistant_cabinet_server.providers.ollama import OllamaProvider

from .conftest import build_settings

QUESTION = {"messages": [{"role": "user", "content": "Bonjour"}]}
#: Stands in for the model's hidden reasoning: it must never appear anywhere the gateway writes.
REASONING = "reasoning-canary-8d41c6f0 the model weighs the question at length"
REQUEST = GenerationRequest(model="test-weights:7b", messages=[Message(role="user", content="Hi")])


def ndjson(*events: dict[str, object]) -> bytes:
    return b"\n".join(json.dumps(event).encode() for event in events) + b"\n"


def answering(text: str, *, reasoning: str = "") -> bytes:
    events: list[dict[str, object]] = []
    if reasoning:
        events.append({"message": {"role": "assistant", "content": "", "thinking": reasoning}})
    if text:
        events.append({"message": {"role": "assistant", "content": text}})
    events.append({"done": True, "prompt_eval_count": 12, "eval_count": 40})
    return ndjson(*events)


class Runtime:
    """A stand-in for the Ollama HTTP API that remembers every chat payload it received."""

    def __init__(self, body: bytes) -> None:
        self.body = body
        self.payloads: list[dict[str, object]] = []

    def __call__(self, request: httpx.Request) -> httpx.Response:
        if request.url.path == "/api/chat":
            self.payloads.append(json.loads(request.content))
            return httpx.Response(200, content=self.body)
        return httpx.Response(404)


def provider_over(
    handler: Callable[[httpx.Request], httpx.Response], *, think: str = "default"
) -> OllamaProvider:
    provider = OllamaProvider("http://ollama.invalid", think=think)  # type: ignore[arg-type]
    provider._client = httpx.AsyncClient(  # noqa: SLF001 - swapping the transport is the point
        base_url="http://ollama.invalid", transport=httpx.MockTransport(handler)
    )
    return provider


async def drain(provider: OllamaProvider) -> None:
    async for _ in provider.generate(REQUEST):
        pass


@pytest.fixture
def propagating_package_logger() -> Iterator[None]:
    logger = logging.getLogger(PACKAGE_LOGGER_NAME)
    previous = logger.propagate
    logger.propagate = True
    try:
        yield
    finally:
        logger.propagate = previous


# --- the switch ------------------------------------------------------------------------------


async def test_off_asks_the_runtime_not_to_think_at_the_top_level() -> None:
    runtime = Runtime(answering("Bonjour."))

    await drain(provider_over(runtime, think="off"))

    payload = runtime.payloads[0]
    assert payload["think"] is False
    # Ollama reads `think` beside `options`, not inside them.
    assert "think" not in payload.get("options", {})  # type: ignore[operator]


async def test_the_default_sends_no_think_key_at_all() -> None:
    runtime = Runtime(answering("Bonjour."))

    await drain(provider_over(runtime))

    assert "think" not in runtime.payloads[0]


async def test_think_true_is_never_sent_whatever_the_setting() -> None:
    # A model without the capability answers `think: true` with HTTP 400, so true has no use.
    for setting in ("default", "off"):
        runtime = Runtime(answering("Bonjour."))
        await drain(provider_over(runtime, think=setting))
        assert runtime.payloads[0].get("think") is not True


def test_the_setting_defaults_to_the_models_own_behaviour() -> None:
    assert build_settings().llm_think == "default"


def test_the_setting_reads_off_from_the_environment() -> None:
    assert build_settings(LLM_THINK="off").llm_think == "off"


@pytest.mark.parametrize("value", ["on", "true", "1", "high"])
def test_the_setting_has_no_way_to_turn_thinking_on(value: str) -> None:
    with pytest.raises(ValidationError):
        Settings(LLM_THINK=value)  # type: ignore[arg-type]


def test_the_application_gives_the_setting_to_its_provider() -> None:
    app = create_app(settings=build_settings(LLM_THINK="off"))
    with TestClient(app) as client:
        provider = client.app.state.provider  # type: ignore[attr-defined]

        assert provider._think == "off"  # noqa: SLF001


# --- what the reasoning never becomes --------------------------------------------------------


def client_for(runtime: Runtime) -> TestClient:
    return TestClient(create_app(settings=build_settings(), provider=provider_over(runtime)))


@pytest.mark.usefixtures("propagating_package_logger")
def test_the_reasoning_is_counted_and_never_kept_or_logged(
    caplog: pytest.LogCaptureFixture,
) -> None:
    caplog.set_level(logging.DEBUG)
    runtime = Runtime(answering("Bonjour.", reasoning=REASONING))

    with client_for(runtime) as client:
        response = client.post("/v1/chat/completions", json=QUESTION)
        entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]

    assert response.status_code == 200
    assert response.json()["choices"][0]["message"]["content"] == "Bonjour."
    assert entry.thinking_chars == len(REASONING)
    assert entry.outcome == "completed"
    assert "reasoning-canary" not in response.text
    assert "reasoning-canary" not in entry.model_dump_json()
    assert "reasoning-canary" not in caplog.text


# --- an answer that is nothing ---------------------------------------------------------------


def test_a_model_that_only_thought_is_said_aloud_not_returned_blank() -> None:
    runtime = Runtime(answering("", reasoning=REASONING))

    with client_for(runtime) as client:
        response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.status_code == 502
    error = response.json()["error"]
    assert error["code"] == "empty_answer"
    # A machine code, never a sentence for a human, and nothing the model wrote.
    assert error["message"] == "empty_answer"
    assert "reasoning-canary" not in response.text


def test_an_answer_of_blanks_is_empty_too() -> None:
    runtime = Runtime(answering("  \n "))

    with client_for(runtime) as client:
        response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.status_code == 502
    assert response.json()["error"]["code"] == "empty_answer"


def test_a_streamed_empty_answer_ends_in_the_error_event_not_in_a_stop() -> None:
    runtime = Runtime(answering("", reasoning=REASONING))

    with client_for(runtime) as client:
        with client.stream(
            "POST", "/v1/chat/completions", json={**QUESTION, "stream": True}
        ) as response:
            events = [
                line.removeprefix("data: ")
                for line in response.iter_lines()
                if line.startswith("data: ")
            ]

    assert events[-1] == "[DONE]"
    payloads = [json.loads(event) for event in events[:-1]]
    assert payloads[-1]["error"]["code"] == "empty_answer"
    finish_reasons = [
        choice.get("finish_reason") for payload in payloads for choice in payload.get("choices", [])
    ]
    assert "stop" not in finish_reasons
    assert "reasoning-canary" not in " ".join(events)


def test_an_empty_answer_is_registered_as_a_failure_with_its_reasoning_counted() -> None:
    runtime = Runtime(answering("", reasoning=REASONING))

    with client_for(runtime) as client:
        client.post("/v1/chat/completions", json=QUESTION)
        entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]

    assert entry.outcome == "empty_answer"
    assert entry.completion_chars == 0
    assert entry.thinking_chars == len(REASONING)


def test_a_streamed_empty_answer_is_registered_the_same_way() -> None:
    runtime = Runtime(answering("", reasoning=REASONING))

    with client_for(runtime) as client:
        with client.stream(
            "POST", "/v1/chat/completions", json={**QUESTION, "stream": True}
        ) as response:
            response.read()
        entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]

    assert entry.outcome == "empty_answer"
    assert entry.thinking_chars == len(REASONING)


def test_a_real_answer_is_not_mistaken_for_an_empty_one() -> None:
    runtime = Runtime(answering("Oui."))

    with client_for(runtime) as client:
        response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.status_code == 200
    assert response.json()["choices"][0]["message"]["content"] == "Oui."
