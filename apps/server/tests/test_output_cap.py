"""An answer has a ceiling, whether or not the caller asked for one.

This exists because of a real incident rather than a hypothetical: a 0.5B model fell into a
repetition loop and wrote the same invented block for three and a half minutes, and nothing in the
chain stopped it (`docs/TROUBLESHOOTING.md`, 22 September 2026). The request timeout could not - it
is the longest allowed gap between chunks, and a steady loop never leaves a gap. So the bound has
to be on the length of the answer, and it has to apply to a request that says nothing about length,
which is every request the product itself makes.
"""

from __future__ import annotations

from fastapi.testclient import TestClient

from assistant_cabinet_server.api.chat import capped_output_tokens
from assistant_cabinet_server.main import create_app

from .conftest import FakeProvider, build_settings

QUESTION = {"messages": [{"role": "user", "content": "Resume ce courrier."}]}


def test_a_request_that_asks_for_no_limit_still_gets_one(client: TestClient) -> None:
    response = client.post("/v1/chat/completions", json=QUESTION)

    assert response.status_code == 200
    provider: FakeProvider = client.app.state.provider  # type: ignore[attr-defined]
    assert provider.requests[-1].max_output_tokens == 2_048


def test_the_cap_is_configuration_not_a_constant() -> None:
    provider = FakeProvider()
    app = create_app(settings=build_settings(MAX_OUTPUT_TOKENS=64), provider=provider)
    with TestClient(app) as client:
        client.post("/v1/chat/completions", json=QUESTION)

    assert provider.requests[-1].max_output_tokens == 64


def test_a_caller_asking_for_more_than_the_cap_is_held_to_the_cap() -> None:
    provider = FakeProvider()
    app = create_app(settings=build_settings(MAX_OUTPUT_TOKENS=64), provider=provider)
    with TestClient(app) as client:
        response = client.post("/v1/chat/completions", json={**QUESTION, "max_tokens": 100_000})

    # Not an error: a request for a longer answer than the practice allows is still a reasonable
    # request, it simply gets a shorter answer.
    assert response.status_code == 200
    assert provider.requests[-1].max_output_tokens == 64


def test_a_caller_asking_for_less_is_taken_at_its_word() -> None:
    provider = FakeProvider()
    app = create_app(settings=build_settings(MAX_OUTPUT_TOKENS=2_048), provider=provider)
    with TestClient(app) as client:
        client.post("/v1/chat/completions", json={**QUESTION, "max_tokens": 32})

    assert provider.requests[-1].max_output_tokens == 32


class TestCappedOutputTokens:
    """The clamp on its own, including the values a hostile or sloppy caller sends."""

    def test_nothing_requested_means_the_cap(self) -> None:
        assert capped_output_tokens(None, 2_048) == 2_048

    def test_less_than_the_cap_is_respected(self) -> None:
        assert capped_output_tokens(200, 2_048) == 200

    def test_the_cap_itself_is_allowed(self) -> None:
        assert capped_output_tokens(2_048, 2_048) == 2_048

    def test_more_than_the_cap_becomes_the_cap(self) -> None:
        assert capped_output_tokens(9_999, 2_048) == 2_048

    # `-1` means "generate until you stop" to llama.cpp and to Ollama, so letting it through would
    # reintroduce exactly the incident this cap exists for.
    def test_the_unbounded_sentinel_is_not_honoured(self) -> None:
        assert capped_output_tokens(-1, 2_048) == 2_048

    def test_zero_is_not_honoured_either(self) -> None:
        assert capped_output_tokens(0, 2_048) == 2_048
