# Bundled OCR resources

Not committed, for the same reason as `../binaries/README.md`: `fra.traineddata` is a model
weight, and the Tesseract DLLs and the pdfium library are the third-party binaries that read one.
Run `scripts/fetch-ocr-resources.ps1` from the repository root; it stages all three trees below.

```text
resources/tesseract/       tesseract.exe's runtime DLLs (thirty files, verified minimal - see
                            ../binaries/README.md); bundled to the application root, not here,
                            because tesseract.exe needs them beside itself, not beside the app
resources/tessdata/fra.traineddata   the LSTM French model Tesseract loads at recognition time
resources/pdfium/pdfium.dll          the platform's prebuilt pdfium dynamic library, loaded
                                      directly by our own process through an explicit path
```

`Rasterizer::new` (`src/raster.rs`) and `TesseractProvider::new` (`src/ocr/tesseract.rs`) resolve
their paths through `app.path().resolve(...)` at runtime, never a literal, and report
`OcrError::EngineUnavailable` rather than panicking when a resource is missing - the product still
works as Sprint 2a without them.

**Verified in this session, with `bundle.externalBin`/`bundle.resources` temporarily in place:**
`cargo tauri build --debug` produced a working MSI and NSIS installer with
`resources/tessdata/fra.traineddata` and `resources/pdfium/pdfium.dll` staged at those exact paths
under the built application, and the Tesseract DLLs staged at the application root beside the
`tesseract.exe` sidecar. That config is not currently in `tauri.conf.json` - see
`../binaries/README.md` for the exact block to restore and why it waits on a macOS fetch script
and a CI step, not on anything wrong with the resources themselves.

See `docs/SPRINT-2.5-ASSESSMENT.md` sections E and L for where these files come from and their
licences (`fra.traineddata`: Apache 2.0; pdfium: BSD-3-Clause / Apache-2.0; Tesseract: Apache 2.0).
`macOS` equivalents are not yet scripted - see `../binaries/README.md`.

# Bundled knowledge packs

`resources/knowledge/` holds the lexicon packs of the knowledge base (`src/knowledge/packs.rs`). They are text,
committed, and embedded into the application with `include_str!` exactly like `resources/tabular-questions/`:
nothing to fetch, nothing platform-specific, the same file on Windows and macOS.

```text
resources/knowledge/base/<locale>.json              always loaded: profession-neutral
resources/knowledge/packs/<pack-id>/<locale>.json   optional: adds vocabulary for one domain
                                                    (health-fr, health-ch, legal-fr, accounting-fr)
```

A pack id is `<domain>-<country>` (`health-fr`, `health-ch`, later `health-eu`, `legal-ch`...), and **modules are never
mixed**: `health-ch` is the Swiss module, chosen **instead of** `health-fr` (« Réglages » -> « Module santé »), not added
to it. Each is complete on its own (its own numbers, `nir` or `avs`; its own organisations; the patient and practitioner
vocabulary). `health-fr` is on by default. The locale of a file (`fr-FR`, `en-US`) is the language of its words; the id
carries the country. The product serves health professionals in France and in Switzerland; any further country is
another pack (`docs/DECISIONS.md`, "Knowledge packs are named domain-country").

A pack holds **words and patterns only, never a sentence a person reads** (a sentence is an interface catalogue
entry). Every file has `schema: 1`, its own `id` and `locale`, and these optional fields:

```text
titles            { person: [...] }    honorifics: "Docteur", "Mrs"
weak_titles       [...]                titles that are also ordinary words or initials: "Me", "M"
particles         [...]                small words inside a name: "de", "van"
title_spoken      { "dr": "docteur" }  how a title is said aloud (words, for text to speech)
stop_words        [...]                months, document words, null-like values; never names
org_markers       { prefix, suffix }   "association", "clinique" / "SARL", "Inc"
identifier_schemes [ { id, kind, regex, normalise: alnum|digits, strength: exact|possible, personal_key } ]
columns           [ { headers: [...], semantic: { type, subtype?, role? }, confidence } ]
relations         [ { from_role, to_role, predicate } ]
roles             [ { id, label_key } ]   label_key is an interface catalogue key under "knowledge."
detection_terms   [...]                   words that point at a domain (activity profiles)
```

Rules the loader enforces (a violation is the machine code `knowledge_pack_invalid`, with the pack id and the place
in the file, and a test that loads every pack in both languages fails the build): no unknown field; a list entry is
a word or a phrase of at most four words with no sentence punctuation; a pattern compiles, matches something and
matches nothing empty; a role used by a column or a relation exists; a domain pack never changes an entity type
(one header cannot mean two types across the active packs); every role `label_key` exists in both interface
catalogues. Edit a pack, run `cargo test --test knowledge_packs`, and
`cargo test --test knowledge_packs print_the_summary_of_every_pack -- --nocapture` to see the counts.
