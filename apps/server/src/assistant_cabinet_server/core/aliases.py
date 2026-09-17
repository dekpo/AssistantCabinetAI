"""Model aliases.

A client asks for `cabinet-chat`. What that resolves to is configuration, so a better model
replaces the current one by editing the alias map, never the code. An alias outside the map is
refused: the gateway serves an allow-list, not whatever the runtime happens to hold.
"""

from __future__ import annotations

from .config import Settings
from .errors import ErrorCode, GatewayError


def resolve_model_alias(settings: Settings, requested: str | None) -> tuple[str, str]:
    """Return `(alias, runtime model name)`."""
    alias = (requested or settings.default_model_alias).strip()
    runtime_model = settings.model_aliases.get(alias)
    if runtime_model is None:
        raise GatewayError(
            ErrorCode.model_alias_not_allowed,
            status_code=400,
            data={"requested": alias, "allowed": settings.allowed_aliases},
        )
    return alias, runtime_model
