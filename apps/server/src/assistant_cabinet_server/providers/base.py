"""The `AIProvider` port.

Everything the product needs from an inference runtime, and nothing a specific runtime happens
to offer. Replacing Ollama with llama.cpp or vLLM means writing one more implementation here,
not touching the API layer.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Literal, Protocol, runtime_checkable

from pydantic import BaseModel, ConfigDict, Field

Role = Literal["system", "user", "assistant"]


class Message(BaseModel):
    model_config = ConfigDict(extra="forbid")

    role: Role
    content: str


class GenerationRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    #: Resolved runtime model name. Aliases are resolved before reaching a provider.
    model: str
    messages: list[Message]
    temperature: float | None = None
    max_output_tokens: int | None = None


class GenerationChunk(BaseModel):
    model_config = ConfigDict(extra="forbid")

    delta: str = ""
    done: bool = False
    prompt_tokens: int | None = None
    completion_tokens: int | None = None


class EmbeddingRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    model: str
    inputs: list[str]


class EmbeddingResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    vectors: list[list[float]]
    prompt_tokens: int | None = None


class RerankRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    model: str
    query: str
    documents: list[str]
    top_n: int | None = None


class RankedDocument(BaseModel):
    model_config = ConfigDict(extra="forbid")

    index: int
    score: float


class RerankResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    ranking: list[RankedDocument]


class ProviderHealth(BaseModel):
    model_config = ConfigDict(extra="forbid")

    reachable: bool
    latency_ms: int | None = None
    #: How many models the runtime holds. Not which ones: a client sees aliases only.
    model_count: int | None = None
    error_code: str | None = Field(default=None, description="Machine code when unreachable.")


@runtime_checkable
class AIProvider(Protocol):
    """Chat, embeddings and reranking, without retention."""

    name: str

    def generate(self, request: GenerationRequest) -> AsyncIterator[GenerationChunk]:
        """Stream the answer. Non-streaming callers aggregate the chunks."""
        ...

    async def embed(self, request: EmbeddingRequest) -> EmbeddingResult: ...

    async def rerank(self, request: RerankRequest) -> RerankResult: ...

    async def check_health(self) -> ProviderHealth: ...

    async def aclose(self) -> None: ...
