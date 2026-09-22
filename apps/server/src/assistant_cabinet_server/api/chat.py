"""`POST /v1/chat/completions` - OpenAI-compatible, streaming or not.

The route resolves an alias, resolves the output locale, assembles the English system prompt with
the rendered output-language directive, and streams the runtime's answer back. It keeps the text
in memory for the duration of the request and records metadata only.
"""

from __future__ import annotations

import json
import time
from collections.abc import AsyncIterator, Callable
from datetime import UTC, datetime
from typing import Literal
from uuid import uuid4

from fastapi import APIRouter
from fastapi.responses import StreamingResponse

from ..core.aliases import resolve_model_alias
from ..core.errors import ErrorCode, GatewayError
from ..core.locales import LocalePack
from ..core.prompts import build_system_prompt
from ..core.register import build_entry
from ..providers import GenerationChunk, GenerationRequest, Message
from .dependencies import ActorDep, LocalesDep, ProviderDep, RegisterDep, SettingsDep
from .schemas import (
    ChatCompletionChoice,
    ChatCompletionChunk,
    ChatCompletionChunkChoice,
    ChatCompletionDelta,
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatMessage,
    Usage,
)

router = APIRouter(prefix="/v1", tags=["chat"])

STREAM_MEDIA_TYPE = "text/event-stream"
STREAM_TERMINATOR = "data: [DONE]\n\n"

FinishReason = Literal["stop", "length"]
#: `(completion text, usage, outcome)`. The text is hashed, never stored.
RecordCallback = Callable[[str, "Usage", str], None]


def assemble_messages(pack: LocalePack, requested: list[ChatMessage]) -> list[Message]:
    """Our English system prompt first, then whatever the client sent, unchanged."""
    messages = [Message(role="system", content=build_system_prompt(pack))]
    messages.extend(Message(role=message.role, content=message.content) for message in requested)
    return messages


def enforce_context_cap(messages: list[Message], limit: int) -> str:
    """Refuse an oversized request and return the assembled text for hashing.

    The client caps too, in Rust. The gateway caps again because it does not trust a caller.
    """
    prompt_text = "\n".join(message.content for message in messages)
    if len(prompt_text) > limit:
        raise GatewayError(
            ErrorCode.context_too_large,
            status_code=413,
            data={"chars": len(prompt_text), "limit": limit},
        )
    return prompt_text


def capped_output_tokens(requested: int | None, limit: int) -> int:
    """The ceiling actually applied to one answer.

    A caller may ask for less, never for more, and a caller that asks for nothing gets the cap
    rather than no limit at all - that last case is the one that matters, since no client of ours
    sends `max_tokens`. Anything outside the range, including the `-1` that means "unbounded" to
    several runtimes, is read as no request at all: the gateway does not trust the caller here any
    more than it does on the context cap.

    Unlike an oversized context this is not refused with an error. A request for a longer answer
    than the practice allows is still a reasonable request; it simply gets a shorter answer.
    """
    if requested is None or not 1 <= requested <= limit:
        return limit
    return requested


async def _chunks(
    first: GenerationChunk | None, rest: AsyncIterator[GenerationChunk]
) -> AsyncIterator[GenerationChunk]:
    if first is not None:
        yield first
    async for chunk in rest:
        yield chunk


def _sse(payload: dict[str, object]) -> str:
    return f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"


@router.post("/chat/completions", response_model=None)
async def create_chat_completion(
    payload: ChatCompletionRequest,
    settings: SettingsDep,
    provider: ProviderDep,
    locales: LocalesDep,
    register: RegisterDep,
    actor: ActorDep,
) -> StreamingResponse | ChatCompletionResponse:
    started_at = datetime.now(tz=UTC)
    completion_id = f"chatcmpl-{uuid4().hex}"
    created = int(time.time())

    alias, runtime_model = resolve_model_alias(settings, payload.model)
    pack = locales.resolve(payload.output_locale, fallback=settings.default_output_locale)
    messages = assemble_messages(pack, payload.messages)
    prompt_text = enforce_context_cap(messages, settings.max_context_chars)

    def record(completion: str, usage: Usage, outcome: str) -> None:
        register.record(
            build_entry(
                request_id=completion_id,
                actor=actor,
                started_at=started_at,
                model_alias=alias,
                output_locale=pack.locale,
                message_count=len(messages),
                prompt_text=prompt_text,
                completion_text=completion,
                prompt_tokens=usage.prompt_tokens or None,
                completion_tokens=usage.completion_tokens or None,
                outcome=outcome,
            )
        )

    generation = provider.generate(
        GenerationRequest(
            model=runtime_model,
            messages=messages,
            temperature=payload.temperature,
            max_output_tokens=capped_output_tokens(payload.max_tokens, settings.max_output_tokens),
        )
    )

    # Pull the first chunk before answering, so an unreachable runtime becomes an HTTP error
    # with a code rather than a broken stream the client cannot explain.
    try:
        first = await anext(generation, None)
    except GatewayError as error:
        record("", Usage(), error.code.value)
        raise

    if payload.stream:
        return StreamingResponse(
            _stream_completion(
                _chunks(first, generation),
                completion_id=completion_id,
                created=created,
                alias=alias,
                record=record,
            ),
            media_type=STREAM_MEDIA_TYPE,
        )

    parts: list[str] = []
    usage = Usage()
    try:
        async for chunk in _chunks(first, generation):
            parts.append(chunk.delta)
            if chunk.done:
                usage = _usage_from(chunk)
    except GatewayError as error:
        record("".join(parts), usage, error.code.value)
        raise

    completion = "".join(parts)
    record(completion, usage, "completed")
    return ChatCompletionResponse(
        id=completion_id,
        created=created,
        model=alias,
        choices=[
            ChatCompletionChoice(message=ChatMessage(role="assistant", content=completion)),
        ],
        usage=usage,
    )


def _usage_from(chunk: GenerationChunk) -> Usage:
    prompt_tokens = chunk.prompt_tokens or 0
    completion_tokens = chunk.completion_tokens or 0
    return Usage(
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
        total_tokens=prompt_tokens + completion_tokens,
    )


async def _stream_completion(
    chunks: AsyncIterator[GenerationChunk],
    *,
    completion_id: str,
    created: int,
    alias: str,
    record: RecordCallback,
) -> AsyncIterator[str]:
    parts: list[str] = []
    usage = Usage()
    outcome = "completed"

    def envelope(delta: ChatCompletionDelta, finish_reason: FinishReason | None = None) -> str:
        chunk = ChatCompletionChunk(
            id=completion_id,
            created=created,
            model=alias,
            choices=[ChatCompletionChunkChoice(delta=delta, finish_reason=finish_reason)],
        )
        return _sse(chunk.model_dump())

    try:
        yield envelope(ChatCompletionDelta(role="assistant", content=""))
        try:
            async for chunk in chunks:
                if chunk.delta:
                    parts.append(chunk.delta)
                    yield envelope(ChatCompletionDelta(content=chunk.delta))
                if chunk.done:
                    usage = _usage_from(chunk)
            yield envelope(ChatCompletionDelta(), finish_reason="stop")
        except GatewayError as error:
            outcome = error.code.value
            yield _sse(error.to_payload())
        yield STREAM_TERMINATOR
    finally:
        record("".join(parts), usage, outcome)
