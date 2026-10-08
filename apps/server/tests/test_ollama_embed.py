"""`OllamaProvider.embed` when the runtime is slow, gone or fine.

The chat path already turned an httpx timeout into a 504 `provider_error` with a `timeout` reason.
The embeddings path caught only a refused connection, so a runtime that took longer than the
provider's deadline surfaced as an unhandled 500 with no code, and the register - which logs the
outcome from the code - never saw a failure to record. These tests drive the real provider over
an in-memory transport, so nothing here needs Ollama.
"""

from __future__ import annotations

import json
import logging
from collections.abc import Callable, Iterator

import httpx
import pytest
from fastapi.testclient import TestClient

from assistant_cabinet_server.core.errors import GatewayError
from assistant_cabinet_server.core.no_store import PACKAGE_LOGGER_NAME
from assistant_cabinet_server.main import create_app
from assistant_cabinet_server.providers import EmbeddingRequest
from assistant_cabinet_server.providers.ollama import OllamaProvider

from .conftest import RECORDED_EMBEDDING_MODEL, build_settings

NONCE = "passage-canary-5c19e0a7"
PASSAGES = {"input": [f"Extrait confidentiel: {NONCE}", "Deuxieme passage."]}


def provider_over(handler: Callable[[httpx.Request], httpx.Response]) -> OllamaProvider:
    provider = OllamaProvider("http://ollama.invalid")
    provider._client = httpx.AsyncClient(  # noqa: SLF001 - swapping the transport is the point
        base_url="http://ollama.invalid", transport=httpx.MockTransport(handler)
    )
    return provider


def slow(request: httpx.Request) -> httpx.Response:
    raise httpx.ReadTimeout("took too long", request=request)


def refused(request: httpx.Request) -> httpx.Response:
    raise httpx.ConnectError("refused", request=request)


def healthy(request: httpx.Request) -> httpx.Response:
    count = len(json.loads(request.content)["input"])
    return httpx.Response(
        200, json={"embeddings": [[0.5, 0.25] for _ in range(count)], "prompt_eval_count": 6}
    )


@pytest.fixture
def propagating_package_logger() -> Iterator[None]:
    logger = logging.getLogger(PACKAGE_LOGGER_NAME)
    previous = logger.propagate
    logger.propagate = True
    try:
        yield
    finally:
        logger.propagate = previous


def client_for(provider: OllamaProvider) -> TestClient:
    return TestClient(create_app(settings=build_settings(), provider=provider))


async def test_a_runtime_that_is_too_slow_is_a_504_not_an_unhandled_error() -> None:
    provider = provider_over(slow)

    with pytest.raises(GatewayError) as raised:
        await provider.embed(EmbeddingRequest(model=RECORDED_EMBEDDING_MODEL, inputs=["un"]))

    assert raised.value.status_code == 504
    assert raised.value.code.value == "provider_error"
    assert raised.value.data == {"provider": "ollama", "reason": "timeout"}


def cut_off(request: httpx.Request) -> httpx.Response:
    raise httpx.ReadError("connection reset", request=request)


def half_sent(request: httpx.Request) -> httpx.Response:
    raise httpx.RemoteProtocolError("server disconnected", request=request)


@pytest.mark.parametrize("runtime", [refused, cut_off, half_sent], ids=["refused", "reset", "cut"])
async def test_a_runtime_that_is_gone_is_a_503_however_the_connection_ended(
    runtime: Callable[[httpx.Request], httpx.Response],
) -> None:
    # Stopping the container during a request resets the connection rather than refusing it; on
    # the live stack that was an unhandled 500, and indexing read it as a problem with one file.
    provider = provider_over(runtime)

    with pytest.raises(GatewayError) as raised:
        await provider.embed(EmbeddingRequest(model=RECORDED_EMBEDDING_MODEL, inputs=["un"]))

    assert raised.value.status_code == 503
    assert raised.value.code.value == "provider_unreachable"


async def test_a_healthy_runtime_still_returns_its_vectors() -> None:
    provider = provider_over(healthy)

    result = await provider.embed(
        EmbeddingRequest(model=RECORDED_EMBEDDING_MODEL, inputs=["un", "deux"])
    )

    assert result.vectors == [[0.5, 0.25], [0.5, 0.25]]
    assert result.prompt_tokens == 6


def test_the_route_answers_a_slow_runtime_with_the_gateways_own_error() -> None:
    with client_for(provider_over(slow)) as client:
        response = client.post("/v1/embeddings", json=PASSAGES)

    assert response.status_code == 504
    error = response.json()["error"]
    assert error["code"] == "provider_error"
    assert error["data"] == {"provider": "ollama", "reason": "timeout"}


def test_a_slow_runtime_is_recorded_as_a_failure_not_a_completion() -> None:
    with client_for(provider_over(slow)) as client:
        client.post("/v1/embeddings", json=PASSAGES)
        entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]

    assert entry.outcome == "provider_error"
    assert entry.vector_count == 0
    assert entry.input_count == 2
    assert NONCE not in entry.model_dump_json()


@pytest.mark.usefixtures("propagating_package_logger")
@pytest.mark.parametrize(
    "runtime", [slow, refused, cut_off, healthy], ids=["timeout", "refused", "reset", "ok"]
)
def test_no_log_record_carries_a_passage_whether_the_call_fails_or_not(
    runtime: Callable[[httpx.Request], httpx.Response], caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.DEBUG)

    with client_for(provider_over(runtime)) as client:
        client.post("/v1/embeddings", json=PASSAGES)

    assert caplog.records, "the call should have been logged as metadata"
    assert NONCE not in caplog.text
    for record in caplog.records:
        assert NONCE not in record.getMessage()
        assert NONCE not in str(record.args)
