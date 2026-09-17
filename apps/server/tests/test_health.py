"""`/health` tells the client what is wrong, as codes."""

from __future__ import annotations

from fastapi.testclient import TestClient

from assistant_cabinet_server.main import create_app

from .conftest import CHAT_ALIAS, FAST_ALIAS, FakeProvider, build_settings


def test_health_is_green_when_the_runtime_answers(client: TestClient) -> None:
    body = client.get("/health").json()

    assert body["status"] == "ok"
    assert body["issues"] == []
    assert body["provider"]["reachable"] is True
    assert body["aliases"] == [CHAT_ALIAS, FAST_ALIAS]
    assert body["default_output_locale"] == "fr-FR"
    assert "fr-FR" in body["output_locales"]


def test_health_is_degraded_with_a_code_when_the_runtime_is_silent() -> None:
    app = create_app(settings=build_settings(), provider=FakeProvider(reachable=False))
    with TestClient(app) as client:
        response = client.get("/health")

    body = response.json()
    # Still 200: the question "is the gateway up" is answered by the status code, and "does the
    # runtime answer" by the body.
    assert response.status_code == 200
    assert body["status"] == "degraded"
    assert body["issues"] == ["provider_unreachable"]


def test_health_reports_a_gateway_without_a_configured_alias() -> None:
    app = create_app(settings=build_settings(MODEL_ALIASES=""), provider=FakeProvider())
    with TestClient(app) as client:
        body = client.get("/health").json()

    assert body["status"] == "degraded"
    assert body["issues"] == ["model_aliases_not_configured"]


def test_the_provider_is_closed_when_the_application_stops() -> None:
    provider = FakeProvider()
    app = create_app(settings=build_settings(), provider=provider)
    with TestClient(app) as client:
        client.get("/health")

    assert provider.closed is True
