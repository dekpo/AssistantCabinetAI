"""Test fixtures.

Every test runs against a fake provider, so nothing here needs Ollama and nothing leaves the
machine. `RECORDED_MODEL` stands in for a weight name: assertions check that it never reaches a
client.
"""

from __future__ import annotations

from collections.abc import AsyncIterator, Iterator

import pytest
from fastapi.testclient import TestClient

from assistant_cabinet_server.core.config import Settings
from assistant_cabinet_server.core.errors import ErrorCode, GatewayError
from assistant_cabinet_server.main import create_app
from assistant_cabinet_server.providers import (
    EmbeddingRequest,
    EmbeddingResult,
    GenerationChunk,
    GenerationRequest,
    ProviderHealth,
    RerankRequest,
    RerankResult,
)

RECORDED_MODEL = "test-weights:7b"
RECORDED_EMBEDDING_MODEL = "test-embed:small"
CHAT_ALIAS = "cabinet-chat"
FAST_ALIAS = "cabinet-rapide"
EMBED_ALIAS = "cabinet-embed"
#: What the gateway publishes, sorted the way it sorts its allow-list. One place to change when
#: a test configuration gains an alias, rather than three assertions in three files.
CONFIGURED_ALIASES = sorted([CHAT_ALIAS, FAST_ALIAS, EMBED_ALIAS])
#: What `/health` publishes for the client's chat-profile picker: `CONFIGURED_ALIASES` minus the
#: embedding alias, which is a backend-only concern (`docs/RETRIEVAL.md`).
CONFIGURED_CHAT_ALIASES = sorted([CHAT_ALIAS, FAST_ALIAS])
#: Every fake vector has this many values, so a test can assert the reported dimensions.
EMBEDDING_DIMENSIONS = 4


class FakeProvider:
    """An `AIProvider` that answers from a script and remembers what it was asked."""

    name = "fake"

    def __init__(
        self,
        *,
        reply: str = "Bonjour.",
        reachable: bool = True,
        failure: GatewayError | None = None,
    ) -> None:
        self.reply = reply
        self.reachable = reachable
        self.failure = failure
        self.requests: list[GenerationRequest] = []
        self.embed_requests: list[EmbeddingRequest] = []
        #: Set to return fewer vectors than inputs, which is the misalignment the route refuses.
        self.embed_vectors_returned: int | None = None
        self.closed = False

    async def generate(self, request: GenerationRequest) -> AsyncIterator[GenerationChunk]:
        self.requests.append(request)
        if self.failure is not None:
            raise self.failure
        for word in self.reply.split(" "):
            yield GenerationChunk(delta=f"{word} ")
        yield GenerationChunk(done=True, prompt_tokens=11, completion_tokens=7)

    async def embed(self, request: EmbeddingRequest) -> EmbeddingResult:
        self.embed_requests.append(request)
        if self.failure is not None:
            raise self.failure
        count = (
            len(request.inputs)
            if self.embed_vectors_returned is None
            else self.embed_vectors_returned
        )
        return EmbeddingResult(
            vectors=[[0.25] * EMBEDDING_DIMENSIONS for _ in range(count)],
            prompt_tokens=3 * len(request.inputs),
        )

    async def rerank(self, request: RerankRequest) -> RerankResult:
        raise GatewayError(
            ErrorCode.provider_capability_unsupported,
            status_code=501,
            data={"provider": self.name, "capability": "rerank"},
        )

    async def check_health(self) -> ProviderHealth:
        if not self.reachable:
            return ProviderHealth(reachable=False, error_code=ErrorCode.provider_unreachable.value)
        return ProviderHealth(reachable=True, latency_ms=3, model_count=2)

    async def aclose(self) -> None:
        self.closed = True

    @property
    def last_system_prompt(self) -> str:
        return self.requests[-1].messages[0].content


def build_settings(**overrides: object) -> Settings:
    defaults: dict[str, object] = {
        "LLM_BASE_URL": "http://127.0.0.1:11434",
        "MODEL_ALIASES": (
            f"{CHAT_ALIAS}={RECORDED_MODEL},"
            f"{FAST_ALIAS}={RECORDED_MODEL},"
            f"{EMBED_ALIAS}={RECORDED_EMBEDDING_MODEL}"
        ),
        "DEFAULT_MODEL_ALIAS": CHAT_ALIAS,
        "DEFAULT_EMBEDDING_ALIAS": EMBED_ALIAS,
        "DEFAULT_OUTPUT_LOCALE": "fr-FR",
    }
    defaults.update(overrides)
    return Settings(**defaults)  # type: ignore[arg-type]


@pytest.fixture
def provider() -> FakeProvider:
    return FakeProvider()


@pytest.fixture
def client(provider: FakeProvider) -> Iterator[TestClient]:
    app = create_app(settings=build_settings(), provider=provider)
    with TestClient(app) as test_client:
        yield test_client
