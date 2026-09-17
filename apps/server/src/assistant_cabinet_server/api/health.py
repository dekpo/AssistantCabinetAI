"""`GET /health` - is the gateway up, and does the inference runtime answer.

Always 200 so that a container health check tests the gateway itself. Whether the runtime
answers is in the body, as codes the client localises.
"""

from __future__ import annotations

from fastapi import APIRouter

from .. import __version__
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

    return HealthResponse(
        status="ok" if not issues else "degraded",
        version=__version__,
        provider=ProviderStatus(name=provider.name, **provider_health.model_dump()),
        aliases=settings.allowed_aliases,
        default_output_locale=settings.default_output_locale,
        output_locales=locales.available,
        issues=issues,
    )
