"""Machine error codes.

The gateway never returns prose for a human. It returns a stable code plus structured data,
and the client localises it. A user-facing sentence here would be a bug: see
docs/LANGUAGE-AND-LOCALE.md.
"""

from __future__ import annotations

from enum import StrEnum
from typing import Any


class ErrorCode(StrEnum):
    """Stable codes. Renaming one is a breaking change for every client."""

    invalid_request = "invalid_request"
    model_alias_not_allowed = "model_alias_not_allowed"
    output_locale_not_supported = "output_locale_not_supported"
    context_too_large = "context_too_large"
    batch_too_large = "batch_too_large"
    provider_unreachable = "provider_unreachable"
    provider_error = "provider_error"
    provider_capability_unsupported = "provider_capability_unsupported"
    internal_error = "internal_error"


class GatewayError(Exception):
    """Raised anywhere in the gateway, rendered as {"error": {"code", "data"}}."""

    def __init__(
        self,
        code: ErrorCode,
        *,
        status_code: int = 400,
        data: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(code.value)
        self.code = code
        self.status_code = status_code
        self.data = data or {}

    def to_payload(self) -> dict[str, Any]:
        # `message` repeats the code so that a third-party OpenAI-compatible client has
        # something to display. It stays a machine code, never a translated sentence.
        return {"error": {"code": self.code.value, "message": self.code.value, "data": self.data}}
