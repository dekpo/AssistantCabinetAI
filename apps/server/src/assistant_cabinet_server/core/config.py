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

    # Embedding weights are not chat weights, so indexing asks for its own alias out of the same
    # allow-list. It must be present in `MODEL_ALIASES` like any other.
    default_embedding_alias: str = Field(default="cabinet-embed", alias="DEFAULT_EMBEDDING_ALIAS")

    # Safety net for a request that arrives without `output_locale`.
    default_output_locale: str = Field(default="fr-FR", alias="DEFAULT_OUTPUT_LOCALE")

    # Context cap. The client caps too, in Rust; the gateway does not trust the caller.
    max_context_chars: int = Field(default=48_000, alias="MAX_CONTEXT_CHARS")

    # Output cap, the counterpart of the one above. Nothing else in the chain bounds how long an
    # answer may be: `max_tokens` is optional in the request, no client of ours sends it, and a
    # read timeout cannot catch a model that loops steadily rather than stalling - a small model
    # once wrote the same invented block for three and a half minutes (`docs/TROUBLESHOOTING.md`,
    # 22 September 2026). Generous for a summary of a specialist report, which measures around
    # 1 300 tokens, and short enough that a repetition loop ends in about a minute.
    max_output_tokens: int = Field(default=2_048, alias="MAX_OUTPUT_TOKENS")

    # Indexing sends batches of chunks rather than one conversation, so embeddings get their own
    # caps. They exist to stop a client sending a whole folder, not to tune throughput.
    max_embedding_chars: int = Field(default=200_000, alias="MAX_EMBEDDING_CHARS")
    max_embedding_inputs: int = Field(default=256, alias="MAX_EMBEDDING_INPUTS")

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

    @property
    def chat_aliases(self) -> list[str]:
        """`allowed_aliases` minus the embedding alias.

        Embeddings are a different job from chat (`docs/RETRIEVAL.md`): the alias exists so
        indexing can go through the same allow-list, never so a human picks it as a chat
        profile. Excluded here rather than filtered client-side, so a client never has to know
        which alias is "the odd one out".
        """
        return [alias for alias in self.allowed_aliases if alias != self.default_embedding_alias]


@lru_cache
def get_settings() -> Settings:
    return Settings()  # type: ignore[call-arg]
