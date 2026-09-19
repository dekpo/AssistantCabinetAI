"""Request register - metadata only.

The register answers "who asked what model, when, and how big was it", never "what did it say".
`extra="forbid"` makes the field list an allow-list: a later commit cannot slip a prompt body in
without the model rejecting it.
"""

from __future__ import annotations

import logging
from collections import deque
from collections.abc import Sequence
from datetime import UTC, datetime
from typing import ClassVar

from pydantic import BaseModel, ConfigDict

from .no_store import log_metadata, sha256_text

logger = logging.getLogger(__name__)


class RegisterEntry(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    #: The log event name. A class attribute, so the register never inspects types to find it.
    event: ClassVar[str] = "request"

    request_id: str
    actor: str
    started_at: datetime
    duration_ms: int
    model_alias: str
    output_locale: str
    message_count: int
    prompt_chars: int
    prompt_sha256: str
    completion_chars: int
    completion_sha256: str
    prompt_tokens: int | None = None
    completion_tokens: int | None = None
    #: `completed`, or the machine error code that ended the request.
    outcome: str


class EmbeddingRegisterEntry(BaseModel):
    """What was vectorised, never what it said.

    Separate from `RegisterEntry` rather than a widened version of it, so that each closed field
    list stays the exact shape of one route.
    """

    model_config = ConfigDict(extra="forbid", frozen=True)

    event: ClassVar[str] = "embedding"

    request_id: str
    actor: str
    started_at: datetime
    duration_ms: int
    model_alias: str
    input_count: int
    input_chars: int
    inputs_sha256: str
    vector_count: int
    dimensions: int | None = None
    prompt_tokens: int | None = None
    #: `completed`, or the machine error code that ended the request.
    outcome: str


Entry = RegisterEntry | EmbeddingRegisterEntry


class RequestRegister:
    """Bounded in-memory register. Nothing is written to disk in v0."""

    def __init__(self, capacity: int) -> None:
        self._entries: deque[Entry] = deque(maxlen=capacity)

    def record(self, entry: Entry) -> None:
        self._entries.append(entry)
        log_metadata(logger, type(entry).event, entry.model_dump(mode="json"))

    @property
    def entries(self) -> list[Entry]:
        return list(self._entries)


def build_entry(
    *,
    request_id: str,
    actor: str,
    started_at: datetime,
    model_alias: str,
    output_locale: str,
    message_count: int,
    prompt_text: str,
    completion_text: str,
    prompt_tokens: int | None,
    completion_tokens: int | None,
    outcome: str,
) -> RegisterEntry:
    """Turn a finished request into metadata. The texts leave as lengths and hashes only."""
    duration_ms = int((datetime.now(tz=UTC) - started_at).total_seconds() * 1000)
    return RegisterEntry(
        request_id=request_id,
        actor=actor,
        started_at=started_at,
        duration_ms=duration_ms,
        model_alias=model_alias,
        output_locale=output_locale,
        message_count=message_count,
        prompt_chars=len(prompt_text),
        prompt_sha256=sha256_text(prompt_text),
        completion_chars=len(completion_text),
        completion_sha256=sha256_text(completion_text),
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
        outcome=outcome,
    )


def build_embedding_entry(
    *,
    request_id: str,
    actor: str,
    started_at: datetime,
    model_alias: str,
    inputs: Sequence[str],
    vector_count: int,
    dimensions: int | None,
    prompt_tokens: int | None,
    outcome: str,
) -> EmbeddingRegisterEntry:
    """Turn a finished embedding request into metadata. The texts leave as counts and one hash."""
    duration_ms = int((datetime.now(tz=UTC) - started_at).total_seconds() * 1000)
    return EmbeddingRegisterEntry(
        request_id=request_id,
        actor=actor,
        started_at=started_at,
        duration_ms=duration_ms,
        model_alias=model_alias,
        input_count=len(inputs),
        input_chars=sum(len(text) for text in inputs),
        inputs_sha256=sha256_text("\n".join(inputs)),
        vector_count=vector_count,
        dimensions=dimensions,
        prompt_tokens=prompt_tokens,
        outcome=outcome,
    )
