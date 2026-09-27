"""The context window one request gets.

The configured window for the alias (`MODEL_CONTEXT_WINDOWS`, else `DEFAULT_CONTEXT_WINDOW`),
never more than the model itself supports. The chat route sends it to the runtime; `/health`
publishes it, so the desktop app can fit the conversation's memory to the model it is about to
ask (docs/SELECTION-AND-MEMORY.md).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from .config import Settings

if TYPE_CHECKING:
    from ..providers import AIProvider


async def effective_context_window(
    settings: Settings, provider: AIProvider, alias: str, runtime_model: str
) -> int:
    configured = settings.context_window_for(alias)
    limit = await provider.context_limit(runtime_model)
    return configured if limit is None else min(configured, limit)
