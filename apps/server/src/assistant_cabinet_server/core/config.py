"""Gateway settings.

Nothing here is a constant of the product: the inference base URL, the alias map, the default
output locale and the caps all come from the environment. Model weight names appear only in
configuration, never in code outside `providers/`.
"""

from __future__ import annotations

import json
from functools import lru_cache
from typing import Annotated

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, NoDecode, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=None, extra="ignore")

    # Inference runtime, reachable from the gateway only. Never exposed to a client.
    llm_base_url: str = Field(default="http://127.0.0.1:11434", alias="LLM_BASE_URL")
    llm_request_timeout_seconds: float = Field(default=180.0, alias="LLM_REQUEST_TIMEOUT_SECONDS")
    llm_health_timeout_seconds: float = Field(default=3.0, alias="LLM_HEALTH_TIMEOUT_SECONDS")

    # Allow-list of aliases the gateway will serve, as `alias=model,alias=model` or as JSON.
    # An empty map means the gateway serves nothing: a missing configuration must fail loudly
    # rather than fall back to some model that happens to be installed.
    # `NoDecode`: the value is read as written, because the settings library would otherwise
    # insist on JSON and the pair form below is what Compose can express.
    model_aliases: Annotated[dict[str, str], NoDecode] = Field(
        default_factory=dict, alias="MODEL_ALIASES"
    )
    default_model_alias: str = Field(default="cabinet-chat", alias="DEFAULT_MODEL_ALIAS")

    # Safety net for a request that arrives without `output_locale`.
    default_output_locale: str = Field(default="fr-FR", alias="DEFAULT_OUTPUT_LOCALE")

    # Context cap. The client caps too, in Rust; the gateway does not trust the caller.
    max_context_chars: int = Field(default=48_000, alias="MAX_CONTEXT_CHARS")

    register_capacity: int = Field(default=500, alias="REGISTER_CAPACITY")
    log_level: str = Field(default="INFO", alias="LOG_LEVEL")

    @field_validator("model_aliases", mode="before")
    @classmethod
    def _parse_aliases(cls, value: object) -> object:
        """Accept JSON or `alias=model,alias=model`.

        Compose cannot interpolate a default that contains braces, so the pair form is what
        `compose.yaml` uses and the JSON form stays available for a hand-written `.env`.
        """
        if not isinstance(value, str):
            return value
        text = value.strip()
        if not text:
            return {}
        if text.startswith("{"):
            return json.loads(text)
        pairs: dict[str, str] = {}
        for item in text.split(","):
            if not item.strip():
                continue
            alias, separator, model = item.partition("=")
            if not separator or not alias.strip() or not model.strip():
                raise ValueError(f"malformed alias entry: {item!r}")
            pairs[alias.strip()] = model.strip()
        return pairs

    @property
    def allowed_aliases(self) -> list[str]:
        return sorted(self.model_aliases)


@lru_cache
def get_settings() -> Settings:
    return Settings()  # type: ignore[call-arg]
