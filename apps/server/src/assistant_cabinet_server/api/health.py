"""`GET /health` - is the gateway up, and does the inference runtime answer.

Always 200 so that a container health check tests the gateway itself. Whether the runtime
answers is in the body, as codes the client localises.
"""

from __future__ import annotations

from fastapi import APIRouter

from .. import __version__
from ..core.context_window import effective_context_window
from .dependencies import LocalesDep, ProviderDep, SettingsDep
from .schemas import HealthResponse, ProviderStatus

router = APIRouter(tags=["health"])


@router.get("/health", response_model=HealthResponse)
async def read_health(
    settings: SettingsDep,
    provider: ProviderDep,
    locales: LocalesDep,
) -> HealthResponse:
    provider_health = await provider.check_health()
    issues: list[str] = []
    if not provider_health.reachable:
        issues.append(provider_health.error_code or "provider_unreachable")
    if not settings.model_aliases:
        issues.append("model_aliases_not_configured")

    # What each chat alias may read in one request, so the client can fit the conversation's
    # memory to it. The model's own maximum is only asked of a runtime that answers - and only
    # once per model - so a runtime that is down cannot slow this check down.
    context_windows: dict[str, int] = {}
    for alias in settings.chat_aliases:
        runtime_model = settings.model_aliases[alias]
        context_windows[alias] = (
            await effective_context_window(settings, provider, alias, runtime_model)
            if provider_health.reachable
            else settings.context_window_for(alias)
        )

    return HealthResponse(
        status="ok" if not issues else "degraded",
        version=__version__,
        provider=ProviderStatus(name=provider.name, **provider_health.model_dump()),
        aliases=settings.chat_aliases,
        default_model_alias=settings.default_model_alias,
        embedding_alias=settings.default_embedding_alias,
        default_output_locale=settings.default_output_locale,
        output_locales=locales.available,
        context_windows=context_windows,
        max_output_tokens=settings.max_output_tokens,
        issues=issues,
    )
