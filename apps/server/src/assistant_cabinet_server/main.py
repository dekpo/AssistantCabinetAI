"""Application factory.

`create_app` takes its settings and its provider as arguments, so a test builds the same
application against a fake runtime and the container builds it against Ollama.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse

from .api import chat, health, models
from .core.config import Settings, get_settings
from .core.errors import ErrorCode, GatewayError
from .core.locales import LocaleCatalogue
from .core.no_store import NoStoreMiddleware, configure_logging
from .core.register import RequestRegister
from .providers import AIProvider
from .providers.ollama import OllamaProvider


def create_app(*, settings: Settings | None = None, provider: AIProvider | None = None) -> FastAPI:
    settings = settings or get_settings()
    configure_logging(settings.log_level)

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        app.state.settings = settings
        app.state.locales = LocaleCatalogue.load()
        app.state.register = RequestRegister(settings.register_capacity)
        app.state.provider = provider or OllamaProvider(
            settings.llm_base_url,
            request_timeout_seconds=settings.llm_request_timeout_seconds,
            health_timeout_seconds=settings.llm_health_timeout_seconds,
        )
        try:
            yield
        finally:
            await app.state.provider.aclose()

    app = FastAPI(
        title="Assistant Cabinet AI gateway",
        version="0.1.0",
        lifespan=lifespan,
        # No interactive documentation is needed by the clients, and the schema alone is enough
        # for whoever integrates one.
        redoc_url=None,
    )
    app.add_middleware(NoStoreMiddleware)
    app.include_router(health.router)
    app.include_router(models.router)
    app.include_router(chat.router)

    @app.exception_handler(GatewayError)
    async def handle_gateway_error(_: Request, error: GatewayError) -> JSONResponse:
        return JSONResponse(status_code=error.status_code, content=error.to_payload())

    @app.exception_handler(RequestValidationError)
    async def handle_validation_error(_: Request, error: RequestValidationError) -> JSONResponse:
        # Only the field locations travel back. A validation message can quote the value that
        # failed, and that value may be document text.
        fields = [".".join(str(part) for part in item.get("loc", ())) for item in error.errors()]
        return JSONResponse(
            status_code=422,
            content=GatewayError(ErrorCode.invalid_request, data={"fields": fields}).to_payload(),
        )

    return app


app = create_app()
