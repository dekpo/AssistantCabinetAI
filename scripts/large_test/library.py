"""The documents of the `demo` profile: the sandbox's own stream (thirty-nine hand-written documents, then the
documents that cover every designed person and organisation, then the long tail), with the format of each one
decided by a fixed wheel, and transcripts and decks added at the wheel's positions.

It reuses `knowledge_sandbox.build.Library`: the document builders, the cast, the writers, the long documents. What
changes is only which formats are read (the readers of 16 October 2026 are in) and which format a document gets.
"""

from __future__ import annotations

import random
from pathlib import Path

from knowledge_sandbox import build
from knowledge_sandbox import plan as PL

from . import docs_demo as DD

NEW_FORMATS = {"odt", "rtf", "eml", "vtt", "srt", "odp", "pptx"}
# Documents the product reads (the OCR ones through the local engine). `ppt` (old binary) and the spreadsheets
# saved among the documents are not.
READABLE = build.READABLE | NEW_FORMATS
# The sandbox's own hand-written documents come first and keep the formats they were designed with.
HAND_WRITTEN = 39


class DemoLibrary(build.Library):
    READABLE_FORMATS = READABLE
    # Transcripts and decks are placed by the wheel, so the long tail draws none of its own.
    KIND_WEIGHTS = [(kind, weight) for kind, weight in PL.KIND_WEIGHTS if kind not in ("transcription", "presentation")]

    def __init__(self, out: Path, total: int, seed: int, with_long_documents: bool = True):
        super().__init__(out, total, seed)
        self.wheel = DD.Wheel()
        self.codes = DD.Codes()
        self.draws = 0
        self.with_long_documents = with_long_documents
        self.rng_shape = random.Random(f"shape-{seed}")
        # The whole text of every document, for the book (never written anywhere).
        self.texts: dict = {}
        self._last = None

    # -----------------------------------------------------------------------------------------------------------

    def produce(self, kind: str, fn, flags: dict, long_pages: int = 0) -> None:
        def build_and_remember(doc):
            fn(doc)
            self._last = doc

        super().produce(kind, build_and_remember, flags, long_pages)
        doc, record = self._last, self.records[-1]
        parts = [doc.title] + list(doc.paragraphs)
        parts += [f"{label} : {text}" for _, _, label, text in doc.cues]
        parts += [" ".join([heading] + bullets) for heading, bullets in doc.slides]
        if doc.table:
            parts += [" ".join(str(cell) for cell in row) for row in doc.table]
        self.texts[record["path"]] = "\n".join(part for part in parts if part)
        if record["format"] in ("pptx", "odp"):
            record["pages"] = len(doc.slides)
        if doc.cues:
            record["cues"] = len(doc.cues)

    def _maybe_long(self) -> None:
        if self.with_long_documents:
            super()._maybe_long()

    def special(self, fmt: str):
        """The spec of a document built for its format: a transcript, or a deck."""
        rng = self.rng_shape
        if fmt in ("vtt", "srt"):
            cues = rng.randint(180, 420)
            return (
                "transcription",
                lambda doc: DD.long_transcript(doc, self.cast, self.codes, cues),
                {"country": "CH" if rng.random() < 0.4 else "FR", "fmt": fmt, "special": "long-transcript"},
            )
        slides = rng.randint(6, 12)
        return (
            "presentation",
            lambda doc: DD.deck(doc, self.cast, self.codes, slides),
            {"country": "CH" if rng.random() < 0.4 else "FR", "fmt": fmt, "special": "deck"},
        )

    def add_template(self) -> None:
        """A letter template with fields, as a practice keeps them: the file-fill plan finds it by the columns its
        fields name. It is an ordinary text document too (the Analyse reads it), placed after the stream."""
        lines = [
            "Objet : Rappel de facture",
            "",
            "Bonjour,",
            "",
            "Nous n'avons pas encore reçu le règlement de la facture du {{Date}} ({{Libellé}}), "
            "d'un montant de {{Montant TTC}}. Son statut est : {{Statut}}.",
            "",
            "Nous vous remercions de bien vouloir régulariser cette situation.",
            "",
            "Cabinet de groupe des Voirons",
        ]
        text = chr(10).join(lines) + chr(10)
        relative = "modeles/modele-relance-facture.txt"
        target = self.out / "documents" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text.encode("utf-8"))
        self.records.append({
            "index": len(self.records) + 1, "path": relative, "format": "txt", "extension": "txt", "kind": "modele",
            "date": "2026-01-05", "country": "FR", "readable": True, "ocr": False, "pages": 1,
            "tier": len(self.records) + 1, "special": "template", "about": [], "mentions": [], "terms": [],
            "ocr_pages": [], "ocr_level": None, "facts": {}, "needles": {},
        })
        self.texts[relative] = text

    def build_documents(self) -> None:
        for kind, fn, flags in PL.plan_documents(self.cast, self.seed, self.total):
            if len(self.records) >= self.total:
                return
            self.produce(kind, fn, flags)
            self._maybe_long()
        coverage = list(PL.coverage_specs(self.cast, self.rng_cov, self.seen, self.next_invoice, self.next_contract))
        while len(self.records) < self.total:
            self.draws += 1
            fmt = self.wheel.format_at(self.draws)
            if fmt in DD.BUILT_FOR_FORMAT:
                kind, fn, flags = self.special(fmt)
            else:
                if coverage:
                    kind, fn, flags = coverage.pop(0)
                else:
                    kind, fn, flags = PL.random_spec(
                        self.cast, self.rng_rand, self.next_invoice, self.next_contract, self.KIND_WEIGHTS
                    )
                flags = {**flags, "fmt": fmt}
            self.produce(kind, fn, flags)
            self._maybe_long()
