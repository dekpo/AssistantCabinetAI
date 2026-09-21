"""Server locale packs.

A pack is data, not code: adding a language means adding a TOML file next to the others.
The packs are the only place in `apps/server` where a language other than English may appear,
because the disclaimer and the file name stems are read by the user, not by the model.
"""

from __future__ import annotations

import tomllib
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field

from .errors import ErrorCode, GatewayError

LOCALES_DIRECTORY = Path(__file__).resolve().parent.parent / "locales"


class LocalePack(BaseModel):
    model_config = ConfigDict(extra="forbid")

    #: BCP 47 tag, taken from the file name.
    locale: str
    #: English name of the language, used to render the output-language directive.
    language_name: str
    #: File name vocabulary the model may propose. Consumed by the naming plan in sprint 3.
    filename_stems: dict[str, str] = Field(default_factory=dict)
    #: Assistance notice. The client appends it from its own catalogue; the model is never
    #: asked to reproduce it. Kept here so a second client gets the same wording.
    disclaimer: str
    #: How the model names a retrieved passage in this language ("excerpt" / "extrait").
    passage_noun: str


class LocaleCatalogue:
    """The packs shipped with the gateway, loaded once at startup."""

    def __init__(self, packs: dict[str, LocalePack]) -> None:
        self._packs = packs

    @classmethod
    def load(cls, directory: Path | None = None) -> LocaleCatalogue:
        directory = directory or LOCALES_DIRECTORY
        packs: dict[str, LocalePack] = {}
        for path in sorted(directory.glob("*.toml")):
            with path.open("rb") as handle:
                raw = tomllib.load(handle)
            packs[path.stem] = LocalePack(locale=path.stem, **raw)
        return cls(packs)

    @property
    def available(self) -> list[str]:
        return sorted(self._packs)

    def resolve(self, locale: str | None, *, fallback: str) -> LocalePack:
        """Return the pack for `locale`, or for `fallback` when the request carried none.

        A missing `output_locale` falls back to the configured default. An unsupported one is
        refused with a code, because answering in the wrong language silently is worse than
        failing.
        """
        requested = (locale or fallback).strip()
        pack = self._match(requested)
        if pack is not None:
            return pack
        raise GatewayError(
            ErrorCode.output_locale_not_supported,
            status_code=400,
            data={"requested": requested, "supported": self.available},
        )

    def _match(self, tag: str) -> LocalePack | None:
        if tag in self._packs:
            return self._packs[tag]
        language = tag.partition("-")[0].lower()
        for candidate in self.available:
            if candidate.partition("-")[0].lower() == language:
                return self._packs[candidate]
        return None
