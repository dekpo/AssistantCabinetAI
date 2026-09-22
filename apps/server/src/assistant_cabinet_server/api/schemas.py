"""Wire shapes.

`/v1/chat/completions` follows the OpenAI request and response bodies so that any compatible
client works, plus one product field, `output_locale`. Unknown fields are ignored rather than
refused: third-party clients send plenty of them, and refusing would break the contract for no
gain.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from ..providers import Role


class ChatMessage(BaseModel):
    model_config = ConfigDict(extra="ignore")

    role: Role
    content: str


class ChatCompletionRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")

    #: A gateway alias, never a weight name. Absent means the configured default alias.
    model: str | None = None
    messages: list[ChatMessage] = Field(min_length=1)
    stream: bool = False
    temperature: float | None = None
    #: A ceiling on the answer, clamped to the gateway's own cap. Absent means the cap itself: an
    #: unbounded answer is not something a caller can ask for.
    max_tokens: int | None = None

    #: BCP 47 tag of the language the answer must be written in. Owned by the client.
    output_locale: str | None = None


class Usage(BaseModel):
    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_tokens: int = 0


class ChatCompletionChoice(BaseModel):
    index: int = 0
    message: ChatMessage
    finish_reason: Literal["stop", "length"] = "stop"


class ChatCompletionResponse(BaseModel):
    id: str
    object: Literal["chat.completion"] = "chat.completion"
    created: int
    model: str
    choices: list[ChatCompletionChoice]
    usage: Usage


class ChatCompletionDelta(BaseModel):
    role: Role | None = None
    content: str | None = None


class ChatCompletionChunkChoice(BaseModel):
    index: int = 0
    delta: ChatCompletionDelta
    finish_reason: Literal["stop", "length"] | None = None


class ChatCompletionChunk(BaseModel):
    id: str
    object: Literal["chat.completion.chunk"] = "chat.completion.chunk"
    created: int
    model: str
    choices: list[ChatCompletionChunkChoice]


class EmbeddingsRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")

    #: A gateway alias, never a weight name. Absent means the configured embedding alias.
    model: str | None = None
    #: One text or a batch. OpenAI allows both; indexing a folder sends batches.
    input: str | list[str]


class EmbeddingVector(BaseModel):
    object: Literal["embedding"] = "embedding"
    index: int
    embedding: list[float]


class EmbeddingUsage(BaseModel):
    prompt_tokens: int = 0
    total_tokens: int = 0


class EmbeddingsResponse(BaseModel):
    object: Literal["list"] = "list"
    data: list[EmbeddingVector]
    model: str
    usage: EmbeddingUsage


class ModelCard(BaseModel):
    id: str
    object: Literal["model"] = "model"
    created: int
    owned_by: str = "assistant-cabinet"


class ModelList(BaseModel):
    object: Literal["list"] = "list"
    data: list[ModelCard]


class ProviderStatus(BaseModel):
    name: str
    reachable: bool
    latency_ms: int | None = None
    model_count: int | None = None
    error_code: str | None = None


class HealthResponse(BaseModel):
    status: Literal["ok", "degraded"]
    version: str
    provider: ProviderStatus
    #: Aliases this gateway will serve. The catalogue a client may choose from.
    aliases: list[str]
    #: The chat alias a client should fall back to when its own choice is no longer valid (an
    #: alias renamed or removed server-side). Always a member of `aliases`.
    default_model_alias: str
    #: The embedding alias indexing must use. Never in `aliases` (docs/RETRIEVAL.md): a client
    #: does not choose it, it just needs to know the current name to stay in sync with a rename.
    embedding_alias: str
    default_output_locale: str
    output_locales: list[str]
    #: Machine codes describing why the status is degraded. Empty when everything answers.
    issues: list[str] = Field(default_factory=list)
