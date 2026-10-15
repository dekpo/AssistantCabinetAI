"""`POST /v1/rerank` - the passages the client found, put in order by a reranker.

The gateway only passes them to the reranker service and back: it keeps no text (the register
holds counts and one hash), refuses a batch that is too big, and says in `/health` whether a
reranker is configured.
"""

from __future__ import annotations

import httpx
import pytest
from fastapi.testclient import TestClient

from assistant_cabinet_server.core.errors import ErrorCode, GatewayError
from assistant_cabinet_server.main import create_app
from assistant_cabinet_server.providers import RerankRequest
from assistant_cabinet_server.providers.tei import TeiReranker

from .conftest import FakeProvider, build_settings

DOCUMENTS = [
    "Le courrier de la caisse du mois de mars.",
    "La franchise annuelle est de trois cents francs.",
    "Une phrase sans rapport.",
]


def test_documents_come_back_best_first_with_their_original_index(
    client: TestClient, provider: FakeProvider
):
    provider.rerank_scores = [0.1, 0.9, 0.3]
    response = client.post(
        "/v1/rerank", json={"query": "franchise annuelle", "documents": DOCUMENTS}
    )
    assert response.status_code == 200
    body = response.json()
    assert [item["index"] for item in body["results"]] == [1, 2, 0]
    assert body["results"][0]["relevance_score"] == pytest.approx(0.9)
    assert body["object"] == "list"
    assert provider.rerank_requests[-1].query == "franchise annuelle"
    assert provider.rerank_requests[-1].documents == DOCUMENTS


def test_top_n_keeps_only_the_best(client: TestClient, provider: FakeProvider):
    provider.rerank_scores = [0.1, 0.9, 0.3]
    response = client.post(
        "/v1/rerank", json={"query": "franchise", "documents": DOCUMENTS, "top_n": 2}
    )
    assert [item["index"] for item in response.json()["results"]] == [1, 2]


def test_without_a_reranker_the_answer_is_a_capability_code_not_a_made_up_order(
    client: TestClient,
):
    response = client.post("/v1/rerank", json={"query": "franchise", "documents": DOCUMENTS})
    assert response.status_code == 501
    assert response.json()["error"]["code"] == "provider_capability_unsupported"


def test_a_blank_query_or_document_is_refused(client: TestClient, provider: FakeProvider):
    provider.rerank_scores = [0.5, 0.5]
    for payload in (
        {"query": "  ", "documents": ["a", "b"]},
        {"query": "q", "documents": ["a", " "]},
        {"query": "q", "documents": []},
    ):
        response = client.post("/v1/rerank", json=payload)
        assert response.status_code in (400, 422), payload
    assert provider.rerank_requests == []


def test_a_batch_over_the_cap_is_refused_before_the_reranker_sees_it(provider: FakeProvider):
    provider.rerank_scores = [0.5] * 5
    app = create_app(settings=build_settings(MAX_RERANK_DOCUMENTS="3"), provider=provider)
    with TestClient(app) as small:
        response = small.post(
            "/v1/rerank", json={"query": "q", "documents": ["a", "b", "c", "d", "e"]}
        )
    assert response.status_code == 413
    assert response.json()["error"]["code"] == "batch_too_large"
    assert provider.rerank_requests == []


def test_the_register_holds_counts_and_a_hash_never_the_text(
    client: TestClient, provider: FakeProvider
):
    provider.rerank_scores = [0.1, 0.9, 0.3]
    client.post("/v1/rerank", json={"query": "franchise annuelle", "documents": DOCUMENTS})
    entries = client.app.state.register.entries
    entry = entries[-1]
    dumped = entry.model_dump_json()
    assert entry.event == "rerank"
    assert entry.document_count == 3
    assert "franchise" not in dumped and "caisse" not in dumped
    assert len(entry.inputs_sha256) == 64


def test_health_says_whether_a_reranker_is_there(client: TestClient, provider: FakeProvider):
    assert client.get("/health").json()["rerank_available"] is False
    provider.rerank_scores = [0.5]
    assert client.get("/health").json()["rerank_available"] is True


# ---- the client of the reranker service (Text Embeddings Inference)


def tei_with(handler) -> TeiReranker:
    return TeiReranker(
        "http://reranker:80", transport=httpx.MockTransport(handler), request_timeout_seconds=5.0
    )


async def test_the_service_is_asked_for_a_score_per_text_and_the_ranking_is_read_back():
    seen = {}

    def handler(request: httpx.Request) -> httpx.Response:
        seen["url"] = str(request.url)
        seen["body"] = request.read().decode()
        return httpx.Response(200, json=[{"index": 2, "score": 0.8}, {"index": 0, "score": 0.1}])

    result = await tei_with(handler).rerank(
        RerankRequest(model="rerank", query="franchise", documents=["a", "b", "c"])
    )
    assert seen["url"].endswith("/rerank")
    assert '"texts"' in seen["body"] and '"query"' in seen["body"]
    assert [(item.index, item.score) for item in result.ranking] == [(2, 0.8), (0, 0.1)]


async def test_a_service_that_is_down_is_reported_with_a_code_and_never_with_its_body():
    def handler(request: httpx.Request) -> httpx.Response:
        raise httpx.ConnectError("refused")

    with pytest.raises(GatewayError) as caught:
        await tei_with(handler).rerank(RerankRequest(model="rerank", query="q", documents=["a"]))
    assert caught.value.code == ErrorCode.provider_unreachable


async def test_an_error_status_carries_the_status_only():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(500, text="the query was: franchise annuelle")

    with pytest.raises(GatewayError) as caught:
        await tei_with(handler).rerank(RerankRequest(model="rerank", query="q", documents=["a"]))
    assert caught.value.code == ErrorCode.provider_error
    assert "franchise" not in str(caught.value.to_payload())
