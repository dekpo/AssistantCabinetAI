"""`POST /v1/embeddings` - OpenAI-compatible.

Indexing happens on the workstation. Only the chunks it needs vectors for reach the gateway, they
live in memory for the request, and what is recorded afterwards is how much was embedded, never
what. Swapping this route for a local ONNX embedder later changes nothing else: the client talks
to an `Embedder` interface, and this is one implementation of it.
"""

from __future__ import annotations

from datetime import UTC, datetime
from uuid import uuid4

from fastapi import APIRouter

from ..core.aliases import resolve_model_alias
from ..core.config import Settings
from ..core.errors import ErrorCode, GatewayError
from ..core.register import build_embedding_entry
from ..providers import EmbeddingRequest, EmbeddingResult
from .dependencies import ActorDep, ProviderDep, RegisterDep, SettingsDep
from .schemas import EmbeddingsRequest, EmbeddingsResponse, EmbeddingUsage, EmbeddingVector

router = APIRouter(prefix="/v1", tags=["embeddings"])


def normalise_inputs(payload: EmbeddingsRequest) -> list[str]:
    """One text or a batch, always returned as a batch.

    A blank entry is refused rather than embedded: it would produce a vector that matches
    everything weakly, and it usually means the extractor returned nothing for a page.
    """
    inputs = [payload.input] if isinstance(payload.input, str) else list(payload.input)
    if not inputs or any(not text.strip() for text in inputs):
        raise GatewayError(ErrorCode.invalid_request, data={"fields": ["input"]})
    return inputs


def enforce_embedding_caps(inputs: list[str], settings: Settings) -> None:
    """The client caps too. The gateway caps again because it does not trust a caller."""
    if len(inputs) > settings.max_embedding_inputs:
        raise GatewayError(
            ErrorCode.batch_too_large,
            status_code=413,
            data={"count": len(inputs), "limit": settings.max_embedding_inputs},
        )
    total_chars = sum(len(text) for text in inputs)
    if total_chars > settings.max_embedding_chars:
        raise GatewayError(
            ErrorCode.context_too_large,
            status_code=413,
            data={"chars": total_chars, "limit": settings.max_embedding_chars},
        )


def check_result_matches(result: EmbeddingResult, expected: int) -> int:
    """Refuse a batch that came back the wrong shape.

    A short or ragged batch would silently misalign chunk and vector, and the index would then
    cite the wrong passage forever. Failing here is far cheaper than debugging that later.
    """
    if len(result.vectors) != expected or any(not vector for vector in result.vectors):
        raise GatewayError(
            ErrorCode.provider_error,
            status_code=502,
            data={"reason": "vector_count_mismatch", "expected": expected},
        )
    dimensions = len(result.vectors[0])
    if any(len(vector) != dimensions for vector in result.vectors):
        raise GatewayError(
            ErrorCode.provider_error,
            status_code=502,
            data={"reason": "ragged_vectors"},
        )
    return dimensions


@router.post("/embeddings", response_model=EmbeddingsResponse)
async def create_embeddings(
    payload: EmbeddingsRequest,
    settings: SettingsDep,
    provider: ProviderDep,
    register: RegisterDep,
    actor: ActorDep,
) -> EmbeddingsResponse:
    started_at = datetime.now(tz=UTC)
    request_id = f"embd-{uuid4().hex}"

    alias, runtime_model = resolve_model_alias(
        settings, payload.model or settings.default_embedding_alias
    )
    inputs = normalise_inputs(payload)
    enforce_embedding_caps(inputs, settings)

    def record(vector_count: int, dimensions: int | None, tokens: int | None, outcome: str) -> None:
        register.record(
            build_embedding_entry(
                request_id=request_id,
                actor=actor,
                started_at=started_at,
                model_alias=alias,
                inputs=inputs,
                vector_count=vector_count,
                dimensions=dimensions,
                prompt_tokens=tokens,
                outcome=outcome,
            )
        )

    try:
        result = await provider.embed(EmbeddingRequest(model=runtime_model, inputs=inputs))
        dimensions = check_result_matches(result, len(inputs))
    except GatewayError as error:
        record(0, None, None, error.code.value)
        raise

    record(len(result.vectors), dimensions, result.prompt_tokens, "completed")
    tokens = result.prompt_tokens or 0
    return EmbeddingsResponse(
        data=[
            EmbeddingVector(index=index, embedding=vector)
            for index, vector in enumerate(result.vectors)
        ],
        model=alias,
        usage=EmbeddingUsage(prompt_tokens=tokens, total_tokens=tokens),
    )
