"""No-store policy.

Two mechanisms, because one is a promise and the other is a guard:

1. Responses carry no-store headers, so no intermediary caches an answer.
2. Logging from this package goes through `log_metadata`. Anything else emitted under the
   `assistant_cabinet_server` logger is dropped, so an accidental `logger.info(text)` added
   later cannot leak document text into a log file or a debug trace.
"""

from __future__ import annotations

import hashlib
import json
import logging
from collections.abc import Awaitable, Callable
from typing import Any

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import Response

PACKAGE_LOGGER_NAME = "assistant_cabinet_server"

NO_STORE_RESPONSE_HEADERS = {
    "Cache-Control": "no-store, no-cache, must-revalidate",
    "Pragma": "no-cache",
    "X-Assistant-Cabinet-Store": "none",
}

#: Marker set by `log_metadata`. Records without it never reach a handler.
METADATA_ONLY_ATTRIBUTE = "metadata_only"


class MetadataOnlyFilter(logging.Filter):
    """Keeps only records that were built from validated metadata."""

    def filter(self, record: logging.LogRecord) -> bool:
        return getattr(record, METADATA_ONLY_ATTRIBUTE, False) is True


def configure_logging(level: str) -> None:
    handler = logging.StreamHandler()
    handler.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s"))
    handler.addFilter(MetadataOnlyFilter())

    package_logger = logging.getLogger(PACKAGE_LOGGER_NAME)
    for existing in list(package_logger.handlers):
        package_logger.removeHandler(existing)
    package_logger.addHandler(handler)
    package_logger.setLevel(level.upper())
    # Own the records rather than handing them to the root logger, so the filter above is the
    # only way out of this package.
    package_logger.propagate = False

    # These libraries log request lines; keep them quiet so a URL never grows into a body.
    logging.getLogger("httpx").setLevel(logging.WARNING)
    logging.getLogger("httpcore").setLevel(logging.WARNING)


def log_metadata(logger: logging.Logger, event: str, fields: dict[str, Any]) -> None:
    """Emit one metadata line. `fields` comes from a model with a closed field list."""
    logger.info(
        "%s %s",
        event,
        json.dumps(fields, sort_keys=True, default=str),
        extra={METADATA_ONLY_ATTRIBUTE: True},
    )


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


class NoStoreMiddleware(BaseHTTPMiddleware):
    async def dispatch(
        self, request: Request, call_next: Callable[[Request], Awaitable[Response]]
    ) -> Response:
        response = await call_next(request)
        response.headers.update(NO_STORE_RESPONSE_HEADERS)
        return response
