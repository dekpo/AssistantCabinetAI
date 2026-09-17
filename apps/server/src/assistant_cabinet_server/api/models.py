"""`GET /v1/models` - the alias catalogue.

Aliases only. A weight name never reaches a client, which is what lets the model behind an alias
change without touching any of them.
"""

from __future__ import annotations

import time

from fastapi import APIRouter

from .dependencies import SettingsDep
from .schemas import ModelCard, ModelList

router = APIRouter(prefix="/v1", tags=["models"])


@router.get("/models", response_model=ModelList)
async def list_models(settings: SettingsDep) -> ModelList:
    created = int(time.time())
    return ModelList(
        data=[ModelCard(id=alias, created=created) for alias in settings.allowed_aliases]
    )
