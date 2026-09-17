"""No user language in the gateway sources.

`apps/server` returns machine codes and English instructions. The user's language lives in the
locale packs and in the client catalogues. A French literal in a module here is a bug, and this
test is what catches it before a reviewer has to.
"""

from __future__ import annotations

import re
from pathlib import Path

from assistant_cabinet_server.core.locales import LocaleCatalogue

SOURCE_ROOT = Path(__file__).resolve().parent.parent / "src"

#: Words that cannot plausibly appear in English source. Whole words only.
FRENCH_MARKERS = re.compile(
    r"\b(le|la|les|des|une|est|erreur|fichier|dossier|médecin|envoi|veuillez|aucune)\b",
    re.IGNORECASE,
)


def _python_sources() -> list[Path]:
    return sorted(path for path in SOURCE_ROOT.rglob("*.py"))


def test_the_sources_are_ascii() -> None:
    # An accented character in a module is the cheapest sign that a sentence for a human slipped
    # into code meant for a machine.
    for path in _python_sources():
        text = path.read_text(encoding="utf-8")
        offenders = {character for character in text if ord(character) > 127}
        assert not offenders, f"{path.name} contains non-ASCII characters: {sorted(offenders)}"


def test_no_french_literal_in_the_sources() -> None:
    for path in _python_sources():
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            match = FRENCH_MARKERS.search(line)
            assert match is None, f"{path.name}:{number} looks French: {match.group(0)!r}"


def test_every_pack_carries_a_language_name_and_a_notice() -> None:
    catalogue = LocaleCatalogue.load()

    assert catalogue.available == ["en-US", "fr-FR"]
    for locale in catalogue.available:
        pack = catalogue.resolve(locale, fallback=locale)
        assert pack.language_name.isascii(), "the language name is read by the model, in English"
        assert pack.disclaimer.strip(), "the client needs a notice to append to every summary"
        assert pack.filename_stems, "the naming plan proposes stems from the pack, not free text"
