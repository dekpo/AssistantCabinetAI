"""Data isolation: the gateway keeps no document text anywhere.

The nonce below stands for document text. If it can be found in a log record, in the register, in
a file or in a response header, the guarantee is broken and this test fails.
"""

from __future__ import annotations

import json
import logging
from collections.abc import Iterator
from datetime import UTC, datetime
from pathlib import Path

import pytest
from fastapi.testclient import TestClient
from pydantic import ValidationError

import assistant_cabinet_server
from assistant_cabinet_server.core.no_store import (
    PACKAGE_LOGGER_NAME,
    MetadataOnlyFilter,
    log_metadata,
)
from assistant_cabinet_server.core.register import (
    EmbeddingRegisterEntry,
    RegisterEntry,
    build_embedding_entry,
    build_entry,
)

NONCE = "patient-canary-8f3a1c22"
QUESTION = {"messages": [{"role": "user", "content": f"Resume ce courrier: {NONCE}"}]}
PASSAGES = {"input": [f"Extrait du compte rendu: {NONCE}", "Deuxieme passage."]}


@pytest.fixture
def propagating_package_logger() -> Iterator[None]:
    """Let records escape to the root logger so `caplog` sees them *before* filtering.

    Capturing after the filter would only prove the filter works. What matters is that no code
    path hands document text to logging in the first place.
    """
    logger = logging.getLogger(PACKAGE_LOGGER_NAME)
    previous = logger.propagate
    logger.propagate = True
    try:
        yield
    finally:
        logger.propagate = previous


@pytest.mark.usefixtures("propagating_package_logger")
def test_no_log_record_carries_the_request_text(
    client: TestClient, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.DEBUG)

    response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.status_code == 200
    assert NONCE not in caplog.text
    for record in caplog.records:
        assert NONCE not in record.getMessage()
        assert NONCE not in str(record.args)


@pytest.mark.usefixtures("propagating_package_logger")
def test_no_log_record_carries_an_indexed_passage(
    client: TestClient, caplog: pytest.LogCaptureFixture
) -> None:
    # Indexing sends far more document text through the gateway than chatting does, so the same
    # guarantee is checked on that route rather than assumed from the one above.
    caplog.set_level(logging.DEBUG)

    response = client.post("/v1/embeddings", json=PASSAGES)

    assert response.status_code == 200
    assert NONCE not in caplog.text
    for record in caplog.records:
        assert NONCE not in record.getMessage()
        assert NONCE not in str(record.args)


def test_the_answer_is_not_cacheable(client: TestClient) -> None:
    response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.headers["cache-control"] == "no-store, no-cache, must-revalidate"


def test_the_vectors_are_not_cacheable(client: TestClient) -> None:
    response = client.post("/v1/embeddings", json=PASSAGES)

    assert response.headers["cache-control"] == "no-store, no-cache, must-revalidate"


def test_the_register_holds_metadata_and_hashes_only(client: TestClient) -> None:
    client.post("/v1/chat/completions", json=QUESTION)

    entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]
    serialised = json.dumps(entry.model_dump(mode="json"))
    assert NONCE not in serialised
    assert entry.prompt_chars > 0
    assert len(entry.prompt_sha256) == 64
    assert entry.outcome == "completed"
    assert entry.model_alias == "cabinet-chat"


def test_the_embedding_register_holds_counts_and_a_hash_only(client: TestClient) -> None:
    client.post("/v1/embeddings", json=PASSAGES)

    entry = client.app.state.register.entries[-1]  # type: ignore[attr-defined]
    serialised = json.dumps(entry.model_dump(mode="json"))
    assert NONCE not in serialised
    assert entry.input_count == 2
    assert entry.input_chars > 0
    assert len(entry.inputs_sha256) == 64
    assert entry.outcome == "completed"


def test_the_embedding_register_field_list_is_closed() -> None:
    entry = build_embedding_entry(
        request_id="embd-1",
        actor="anonymous",
        started_at=datetime.now(tz=UTC),
        model_alias="cabinet-embed",
        inputs=[NONCE],
        vector_count=1,
        dimensions=4,
        prompt_tokens=3,
        outcome="completed",
    )

    with pytest.raises(ValidationError):
        EmbeddingRegisterEntry(**{**entry.model_dump(), "inputs": [NONCE]})


def test_the_register_field_list_is_closed() -> None:
    entry = build_entry(
        request_id="chatcmpl-1",
        actor="anonymous",
        started_at=datetime.now(tz=UTC),
        model_alias="cabinet-chat",
        output_locale="fr-FR",
        message_count=2,
        prompt_text=NONCE,
        completion_text=NONCE,
        prompt_tokens=1,
        completion_tokens=1,
        outcome="completed",
    )

    with pytest.raises(ValidationError):
        RegisterEntry(**{**entry.model_dump(), "prompt_body": NONCE})


def test_no_file_is_written_during_a_request(
    client: TestClient, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    package_root = Path(assistant_cabinet_server.__file__).resolve().parent
    monkeypatch.chdir(tmp_path)

    client.post("/v1/chat/completions", json=QUESTION)
    client.post("/v1/embeddings", json=PASSAGES)

    assert list(tmp_path.rglob("*")) == []
    for path in package_root.rglob("*"):
        if path.is_file() and path.suffix in {".py", ".toml", ".log", ".json", ".txt", ".db"}:
            assert NONCE not in path.read_text(encoding="utf-8", errors="ignore")


def test_a_log_call_carrying_text_is_dropped_by_the_guard() -> None:
    emitted: list[str] = []

    class Collector(logging.Handler):
        def emit(self, record: logging.LogRecord) -> None:
            emitted.append(record.getMessage())

    handler = Collector()
    handler.addFilter(MetadataOnlyFilter())
    logger = logging.getLogger(f"{PACKAGE_LOGGER_NAME}.guard_test")
    logger.addHandler(handler)
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    try:
        logger.info("careless debug trace: %s", NONCE)
        log_metadata(logger, "request", {"model_alias": "cabinet-chat", "prompt_chars": 42})
    finally:
        logger.removeHandler(handler)

    assert all(NONCE not in message for message in emitted)
    assert any("cabinet-chat" in message for message in emitted)
