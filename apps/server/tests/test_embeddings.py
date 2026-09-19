"""`POST /v1/embeddings`.

The route indexing depends on. What matters is that it resolves an alias like every other route,
that it refuses a batch it cannot line up with its inputs, and that it records counts rather than
chunks.
"""

from __future__ import annotations

from fastapi.testclient import TestClient

from assistant_cabinet_server.main import create_app

from .conftest import (
    CHAT_ALIAS,
    EMBED_ALIAS,
    EMBEDDING_DIMENSIONS,
    RECORDED_EMBEDDING_MODEL,
    FakeProvider,
    build_settings,
)


def test_one_text_comes_back_as_one_vector(client: TestClient) -> None:
    response = client.post("/v1/embeddings", json={"input": "Compte rendu de cardiologie."})

    assert response.status_code == 200
    body = response.json()
    assert body["object"] == "list"
    assert len(body["data"]) == 1
    assert body["data"][0]["index"] == 0
    assert len(body["data"][0]["embedding"]) == EMBEDDING_DIMENSIONS


def test_a_batch_keeps_its_order(client: TestClient) -> None:
    chunks = [f"passage {index}" for index in range(5)]

    response = client.post("/v1/embeddings", json={"input": chunks})

    assert response.status_code == 200
    assert [item["index"] for item in response.json()["data"]] == [0, 1, 2, 3, 4]


def test_the_client_sees_the_alias_and_the_runtime_sees_the_weights(
    client: TestClient, provider: FakeProvider
) -> None:
    response = client.post("/v1/embeddings", json={"input": "un passage"})

    assert response.json()["model"] == EMBED_ALIAS
    assert provider.embed_requests[-1].model == RECORDED_EMBEDDING_MODEL


def test_an_alias_outside_the_allow_list_is_refused(client: TestClient) -> None:
    response = client.post(
        "/v1/embeddings", json={"input": "un passage", "model": "text-embedding-3-large"}
    )

    assert response.status_code == 400
    assert response.json()["error"]["code"] == "model_alias_not_allowed"


def test_the_chat_alias_may_be_asked_for_explicitly(client: TestClient) -> None:
    # The alias map is one allow-list. Which alias embeds is configuration, not a second list.
    response = client.post("/v1/embeddings", json={"input": "un passage", "model": CHAT_ALIAS})

    assert response.status_code == 200
    assert response.json()["model"] == CHAT_ALIAS


def test_a_blank_passage_is_refused_rather_than_embedded(client: TestClient) -> None:
    response = client.post("/v1/embeddings", json={"input": ["un passage", "   "]})

    assert response.status_code == 400
    assert response.json()["error"]["code"] == "invalid_request"


def test_an_empty_batch_is_refused(client: TestClient) -> None:
    response = client.post("/v1/embeddings", json={"input": []})

    assert response.status_code == 400
    assert response.json()["error"]["code"] == "invalid_request"


def test_too_many_passages_are_refused() -> None:
    app = create_app(settings=build_settings(MAX_EMBEDDING_INPUTS=3), provider=FakeProvider())

    with TestClient(app) as client:
        response = client.post("/v1/embeddings", json={"input": ["a", "b", "c", "d"]})

    assert response.status_code == 413
    error = response.json()["error"]
    assert error["code"] == "batch_too_large"
    assert error["data"] == {"count": 4, "limit": 3}


def test_a_batch_of_oversized_passages_is_refused() -> None:
    app = create_app(settings=build_settings(MAX_EMBEDDING_CHARS=20), provider=FakeProvider())

    with TestClient(app) as client:
        response = client.post("/v1/embeddings", json={"input": ["x" * 15, "y" * 15]})

    assert response.status_code == 413
    error = response.json()["error"]
    assert error["code"] == "context_too_large"
    assert error["data"] == {"chars": 30, "limit": 20}


def test_a_runtime_returning_the_wrong_number_of_vectors_is_refused() -> None:
    # Silently accepting a short batch would bind chunk 2 to the vector of chunk 3, and every
    # later citation would point at the wrong passage.
    provider = FakeProvider()
    provider.embed_vectors_returned = 2
    app = create_app(settings=build_settings(), provider=provider)

    with TestClient(app) as client:
        response = client.post("/v1/embeddings", json={"input": ["a", "b", "c"]})

    assert response.status_code == 502
    error = response.json()["error"]
    assert error["code"] == "provider_error"
    assert error["data"]["reason"] == "vector_count_mismatch"


def test_the_register_counts_the_passages_without_keeping_them(client: TestClient) -> None:
    client.post("/v1/embeddings", json={"input": ["alpha", "bravo"]})

    entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]

    assert entry.model_alias == EMBED_ALIAS
    assert entry.input_count == 2
    assert entry.input_chars == 10
    assert entry.vector_count == 2
    assert entry.dimensions == EMBEDDING_DIMENSIONS
    assert entry.outcome == "completed"
    assert len(entry.inputs_sha256) == 64


def test_a_refusal_is_recorded_with_its_code(client: TestClient) -> None:
    provider: FakeProvider = client.app.state.provider  # type: ignore[attr-defined]
    provider.embed_vectors_returned = 0

    client.post("/v1/embeddings", json={"input": ["alpha"]})

    entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]
    assert entry.outcome == "provider_error"
    assert entry.vector_count == 0
