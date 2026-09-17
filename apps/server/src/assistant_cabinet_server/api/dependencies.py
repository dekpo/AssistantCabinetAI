"""Request-scoped access to what the application was built with.

Everything lives on `app.state`, so a test builds an application with a fake provider instead of
patching a module-level singleton.
"""

from __future__ import annotations

from typing import Annotated

from fastapi import Depends, Request

from ..core.config import Settings
from ..core.locales import LocaleCatalogue
from ..core.register import RequestRegister
from ..providers import AIProvider

#: Header carrying who is asking. Per-person keys replace it when auth arrives in sprint 4.
ACTOR_HEADER = "X-Assistant-Cabinet-Actor"
ANONYMOUS_ACTOR = "anonymous"


def get_settings(request: Request) -> Settings:
    return request.app.state.settings


def get_provider(request: Request) -> AIProvider:
    return request.app.state.provider


def get_locales(request: Request) -> LocaleCatalogue:
    return request.app.state.locales


def get_register(request: Request) -> RequestRegister:
    return request.app.state.register


def get_actor(request: Request) -> str:
    return request.headers.get(ACTOR_HEADER, ANONYMOUS_ACTOR)


SettingsDep = Annotated[Settings, Depends(get_settings)]
ProviderDep = Annotated[AIProvider, Depends(get_provider)]
LocalesDep = Annotated[LocaleCatalogue, Depends(get_locales)]
RegisterDep = Annotated[RequestRegister, Depends(get_register)]
ActorDep = Annotated[str, Depends(get_actor)]
