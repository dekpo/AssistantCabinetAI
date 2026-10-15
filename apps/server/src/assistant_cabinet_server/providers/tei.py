"""Text Embeddings Inference as a reranker service.

The only module in the gateway that knows this service exists. It serves a cross-encoder (a
model that reads a question and a passage together and says how well one answers the other)
behind `POST /rerank`. It never logs a request body and never puts an upstream body into an error,
because an upstream body can echo the question.
"""

from __future__ import annotations

import httpx

from ..core.errors import ErrorCode, GatewayError
from .base import RankedDocument, RerankRequest, RerankResult

NAME = "tei"


class TeiReranker:
    def __init__(
        self,
        base_url: str,
        *,
        request_timeout_seconds: float = 60.0,
        transport: httpx.AsyncBaseTransport | None = None,
    ) -> None:
        self._client = httpx.AsyncClient(
            base_url=base_url.rstrip("/"),
            timeout=httpx.Timeout(request_timeout_seconds, connect=10.0),
            transport=transport,
        )

    async def rerank(self, request: RerankRequest) -> RerankResult:
        payload = {
            "query": request.query,
            "texts": request.documents,
            # A passage longer than the model reads is cut by the service rather than refused.
            "truncate": True,
            "raw_scores": False,
        }
        try:
            response = await self._client.post("/rerank", json=payload)
        except (httpx.NetworkError, httpx.RemoteProtocolError) as error:
            raise GatewayError(
                ErrorCode.provider_unreachable, status_code=503, data={"provider": NAME}
            ) from error
        except httpx.TimeoutException as error:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=504,
                data={"provider": NAME, "reason": "timeout"},
            ) from error
        if response.status_code >= 400:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=502,
                data={"provider": NAME, "upstream_status": response.status_code},
            )
        try:
            items = response.json()
            ranking = [
                RankedDocument(index=int(item["index"]), score=float(item["score"]))
                for item in items
            ]
        except (ValueError, KeyError, TypeError) as error:
            raise GatewayError(
                ErrorCode.provider_error,
                status_code=502,
                data={"provider": NAME, "reason": "malformed_response"},
            ) from error
        ranking.sort(key=lambda item: -item.score)
        if request.top_n:
            ranking = ranking[: request.top_n]
        return RerankResult(ranking=ranking)

    async def aclose(self) -> None:
        await self._client.aclose()
