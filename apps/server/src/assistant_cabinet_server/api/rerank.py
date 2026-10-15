"""`POST /v1/rerank` - put the passages a search found in order, best first.

Retrieval finds candidates cheaply (words and meaning); a cross-encoder reads each one together
with the question and says how well it answers it. The passages live in memory for the request;
what is recorded afterwards is how many there were and one hash, never what they said. Without a
reranker service the answer is a capability code, and the client keeps the order it already has.
"""

from __future__ import annotations

from datetime import UTC, datetime
from uuid import uuid4

from fastapi import APIRouter

from ..core.errors import ErrorCode, GatewayError
from ..core.register import build_rerank_entry
from ..providers import RerankRequest
from .dependencies import ActorDep, ProviderDep, RegisterDep, SettingsDep
from .schemas import RerankPayload, RerankResponse, RerankResultItem

router = APIRouter(prefix="/v1", tags=["rerank"])

#: What `model` says in the answer: the reranker is not an alias a client chooses.
RERANK_MODEL = "assistant-rerank"


@router.post("/rerank", response_model=RerankResponse)
async def create_rerank(
    payload: RerankPayload,
    settings: SettingsDep,
    provider: ProviderDep,
    register: RegisterDep,
    actor: ActorDep,
) -> RerankResponse:
    started_at = datetime.now(tz=UTC)
    request_id = f"rank-{uuid4().hex}"

    if not payload.query.strip() or not payload.documents:
        raise GatewayError(ErrorCode.invalid_request, data={"fields": ["query", "documents"]})
    if any(not text.strip() for text in payload.documents):
        raise GatewayError(ErrorCode.invalid_request, data={"fields": ["documents"]})
    if len(payload.documents) > settings.max_rerank_documents:
        raise GatewayError(
            ErrorCode.batch_too_large,
            status_code=413,
            data={"count": len(payload.documents), "limit": settings.max_rerank_documents},
        )
    total_chars = len(payload.query) + sum(len(text) for text in payload.documents)
    if total_chars > settings.max_rerank_chars:
        raise GatewayError(
            ErrorCode.context_too_large,
            status_code=413,
            data={"chars": total_chars, "limit": settings.max_rerank_chars},
        )

    def record(result_count: int, outcome: str) -> None:
        register.record(
            build_rerank_entry(
                request_id=request_id,
                actor=actor,
                started_at=started_at,
                query=payload.query,
                documents=payload.documents,
                result_count=result_count,
                outcome=outcome,
            )
        )

    try:
        result = await provider.rerank(
            RerankRequest(
                model=RERANK_MODEL,
                query=payload.query,
                documents=payload.documents,
                top_n=payload.top_n,
            )
        )
    except GatewayError as error:
        record(0, error.code.value)
        raise

    # A ranking that names a document the request did not hold is a reranker that went wrong:
    # refused, not passed on, because the client would cite the wrong passage.
    count = len(payload.documents)
    if any(not 0 <= item.index < count for item in result.ranking):
        record(0, ErrorCode.provider_error.value)
        raise GatewayError(
            ErrorCode.provider_error,
            status_code=502,
            data={"reason": "index_out_of_range"},
        )
    record(len(result.ranking), "completed")
    return RerankResponse(
        model=RERANK_MODEL,
        results=[
            RerankResultItem(index=item.index, relevance_score=item.score)
            for item in result.ranking
        ],
    )
