# Sprint 2.5 assessment — local OCR

**Written 19 September 2026, after inspecting the repository at commit `c537805`. No code has changed.**
Answers section 21 of `docs/BRIEF-SPRINT-2.5-OCR.md`, which is the recorded direction.

Read with `docs/ROADMAP.md` (dates), `docs/ARCHITECTURE.md` (ports and the `Source` model),
`docs/RETRIEVAL.md` (the pipeline) and `docs/SPRINT-2-ASSESSMENT.md`, whose structure and reasoning this
document continues.

**Confirmed by the owner, 19 September 2026.** Tesseract with pdfium is accepted, and the reason given is
the one that matters: **running on Windows and on macOS is mandatory, not a preference.** The engine must
not tie the product to one operating system. Section E is updated accordingly, and section O is new — a
cross-platform audit of what exists today, which found one real gap.

---

## What the inspection found

Sprint 2a is **done, and eleven days early**. `docs/ROADMAP.md` gave it 24–30 September; commit `c537805`
landed discovery, extraction, chunking, the SQLite index, retrieval and the sourced chat UI on
19 September. The working tree is clean. That slack is the entire reason a Sprint 2.5 can exist without
touching either milestone date, and it should be spent on ingestion rather than pulled forward into
Sprint 3.

Three findings matter more than the rest.

**The scanned-PDF fixture cannot test OCR.** `scripts/gen-sandbox-fixtures.py:80 make_blank_pdf` writes a
page whose content stream is a single PDF comment. There is no image in
`fixtures/gp-sandbox/inbox/2026-03-20_radiographie-scan.pdf` — nothing for an OCR engine to read. It is a
correct fixture for the behaviour it was built for ("report empty, do not guess") and a useless one for
the behaviour Sprint 2.5 adds. New fixtures are required; see section M.

**Emptiness is decided per document, not per page.** `extraction.rs:50` sets `empty` when *every* page
trims to nothing. A letter whose first page is born-digital and whose second page is a scanned appendix
reports `empty: false`, and page two is silently lost — no chunk, no citation, no mention in
`IndexSummary::empty_files`. That is the one existing behaviour Sprint 2.5 must change rather than extend,
and it is a bug today, independently of OCR.

**The file-hash cache already exists and OCR inherits it for free.** `indexing.rs:47` skips any file whose
SHA-256 matches the stored one. No second cache is needed; see section K.

---

## A. Current document ingestion architecture

Everything below is on the workstation, in `apps/desktop/src-tauri/src`. None of these modules imports
`tauri`; `commands.rs` is the only adapter. `apps/server` sees none of it.

```text
work_folder.rs   allow-list, re-validated at every launch
       ↓
discovery.rs     walk, extension allow-list "pdf docx txt md", symlinks refused
       ↓         -> DiscoveredFile { relative_path, absolute_path, extension, size, modified_at }
indexing.rs      per file: SHA-256, skip if unchanged
       ↓
extraction.rs    pdf -> pdf_extract per page | docx -> quick-xml over word/document.xml | txt/md -> read
       ↓         -> ExtractedDocument { relative_path, pages: [ExtractedPage { page_number, text }], empty }
chunking.rs      paragraph groups, 1200 chars max, short trailing chunks merged
       ↓         -> Chunk { chunk_id "path#pN#sM", relative_path, page_number, section, text }
gateway.rs       embed() in batches capped on count and characters
       ↓
index_store.rs   SQLite: documents(relative_path, sha256, empty) + chunks + chunks_fts (FTS5)
       ↓
retrieval.rs     FTS5 lexical + brute-force cosine, merged, capped at 6 chunks / 6000 chars
       ↓         -> Evidence { chunk_id, relative_path, page_number, section, text, score }
commands.rs      ask_with_sources: embed question -> search -> refuse if empty -> gateway -> stream
```

The insertion point for OCR is exactly one module wide. `extraction.rs` is the only place that turns bytes
into text, and it already has the right shape: a per-page list. Everything downstream of `ExtractedPage`
works unchanged whether the text came from a PDF text layer or from an OCR engine — which is the point of
the brief's section 11, and it is already true of this codebase.

## B. Where `OcrProvider` should live

```text
apps/desktop/src-tauri/src/ocr/mod.rs         the port: OcrProvider, OcrInput, OcrPage, OcrError
apps/desktop/src-tauri/src/ocr/tesseract.rs   the first implementation (a sidecar process)
apps/desktop/src-tauri/src/raster.rs          PDF page -> bitmap, needed only to feed the port
```

On the workstation, beside extraction, and **never** in `apps/server`. The rule that decided the tabular
engine decides this one identically: the server must never receive a document, so the code that reads one
cannot live there. A scanned page is a document in its most complete form — a picture of everything on it.

Rust naming: `OcrProvider`, not `OCRProvider`. Clippy's `upper_case_acronyms` lint rejects the second, and
the codebase already spells acronyms this way (`sha256`, `chunk_id`). The brief's section 5 asks the
contract to follow existing project conventions; this is that.

`extraction.rs` takes the provider as a parameter rather than constructing one, so every existing
extraction test keeps running with no provider at all, and the OCR tests inject a fake. That is one
signature change, not a rewrite.

## C. Existing components reused unchanged

| Component | Why it stands |
| --- | --- |
| `chunking.rs` | Consumes `ExtractedPage`. It does not know, and must not know, how the text was obtained |
| `index_store.rs` schema for `chunks` and `chunks_fts` | A chunk is a chunk. Only the `documents` table gains columns (section K) |
| `retrieval.rs` | Ranking, capping and `build_context_turn` are source-agnostic. An OCR chunk competes on the same footing |
| `gateway.rs`, `apps/server` in full | The gateway never learns that OCR exists. Excerpts are excerpts |
| `work_folder.rs` | The allow-list is the allow-list. OCR adds no new path policy and must not be allowed to |
| `core/no_store.py`, `core/register.py` | The isolation guarantee is unchanged and already enforced by `extra="forbid"` plus `MetadataOnlyFilter` |
| `error.rs` mechanism | New variants, same machine-code-plus-data contract. No prose |
| `src/guards/sources.test.ts` | Keeps working, and will catch the likely mistake: a French word in the OCR language mapping |
| `indexing.rs` hash-and-skip loop | Becomes the OCR cache at no cost (section K) |

## D. Components requiring minimal refactoring

Six additive changes. None is a rewrite; together they are roughly a day and a half.

1. **`discovery.rs:14`** — `SUPPORTED_EXTENSIONS` gains `jpg`, `jpeg`, `png`. Two existing tests assert
   PNG is excluded (`only_supported_extensions_are_returned`, and `an_unsupported_extension_is_refused`
   in `extraction.rs`); both flip to asserting the new behaviour. `tif`/`tiff` are one more string in that
   array when a scanner needs them, deliberately not added now.
2. **`extraction.rs`** — `extract` gains an `&dyn OcrProvider` parameter and decides **per page** whether
   the text layer is usable (section H). `ExtractedPage` gains `origin: PageOrigin`. `ExtractedDocument::empty`
   stays, now meaning "no page produced text by any route".
3. **`chunking.rs`** — `Chunk` carries `origin` through from its page. Three lines; the chunk boundaries
   themselves do not change.
4. **`index_store.rs`** — `chunks` gains `origin` and `confidence`; `documents` gains `ocr_engine` and
   `ocr_engine_version` so that changing the engine invalidates like changing the embedding alias does.
   `StoredChunk` and `Evidence` carry them to the interface.
5. **`error.rs` plus both catalogues** — `ocr_unavailable`, `ocr_language_unavailable`, `ocr_failed`,
   `ocr_page_unreadable`. The guard test fails the build if either catalogue is missing an entry.
6. **`indexing.rs`** — `IndexSummary` gains `ocr_files: Vec<String>` and `low_confidence_files: Vec<String>`,
   so the interface can tell her *which* documents were read by a machine rather than copied. This is the
   answer to the brief's section 10: no raw scores on screen, but never a silent OCR either.

`settings.rs` gains nothing mandatory. The OCR language follows the existing `locale`, and the engine
choice is not a user setting in v0 — there is one engine. Adding a setting for a single value would be the
speculative infrastructure `docs/PLATFORM-VISION.md` exists to prevent.

## E. Recommended first local OCR engine

**Tesseract 5, invoked as a bundled Tauri sidecar process, with the LSTM `fra` traineddata shipped as a
resource.**

| Criterion (brief, section 8) | How it scores |
| --- | --- |
| Local and offline | Fully. No network code path exists in the binary |
| Deployment feasibility | Tauri 2 `externalBin` is the documented, supported path. One `.exe` plus its DLL set plus one traineddata file, bundled by the existing installer pipeline |
| Windows compatibility | The pilot's 2019 PC runs it on CPU. No GPU, no runtime, no Python |
| French quality | `fra.traineddata` (LSTM) is the best-documented open French model. Her scans are flatbed captures of printed correspondence, which is the easy case |
| PDF and image support | Images only. PDF pages must be rasterised first — see below |
| Performance | Roughly 0.5–2 s per page on that CPU. Her volume is 10–20 scans a week, and unchanged files are skipped |
| Licensing | Apache 2.0. Unambiguous for a commercial product, and a line in `models/LICENSES.md` like every other weight |
| Ease of replacement | A process boundary is the most replaceable boundary there is, and it makes the port trivially fakeable in tests |

Two arguments decide it over the alternatives, and neither is accuracy.

**We ship the engine, so her machine behaves like the test machine.** Every other candidate either
depends on something already present on her PC that we cannot verify from here, or adds a runtime to the
installer three weeks before deployment.

**It runs on both operating systems, from the same source.** This is the owner's condition, confirmed on
19 September 2026, and it eliminates the otherwise attractive platform engines on its own. Windows.Media.Ocr
would mean a Windows implementation plus an Apple Vision implementation plus a permanent asymmetry in
behaviour, quality and language support between the two clients — two OCR products maintained as one.
Tesseract is one engine, one `fra` model, one set of results. The only platform-specific part is which
prebuilt binary the bundler picks up:

```text
binaries/tesseract-x86_64-pc-windows-msvc.exe     the pilot workstation
binaries/tesseract-aarch64-apple-darwin           an Apple-silicon Mac
binaries/tesseract-x86_64-apple-darwin            an Intel Mac
```

Tauri 2 resolves the target triple itself, so that is configuration in `tauri.conf.json` rather than
`#[cfg]` in the code. `pdfium-render` works the same way: the pdfium-binaries project publishes Windows
x64 and macOS arm64 and x64 builds under the same permissive licence. `fra.traineddata` is one file and
is byte-identical on both.

The single rule that keeps it true: **no `#[cfg(windows)]` anywhere in `src/ocr/`.** If a platform
difference ever has to exist, it belongs in the bundler configuration or in sidecar discovery, never in
the recognition path — otherwise the two clients drift apart and only one of them is ever tested.

Two consequences to accept openly. The installer grows by roughly 50–80 MB, which nobody will notice on a
practice PC. And rasterisation is a second dependency: **`pdfium-render`** (pdfium, BSD-3-Clause and
Apache-2.0) renders a PDF page to a bitmap in memory. It does not replace `pdf-extract`, which keeps doing
native text extraction; it is only there to feed pixels to the port.

Two things Sprint 2.5 must do to keep this honest:

- **Invoke Tesseract through stdin and stdout** (`tesseract - - -l fra`), never with input and output file
  paths. The default CLI form writes recognised text to a `.txt` file on disk. Patient text in a temporary
  file, outside the work folder, with no trash policy, would be a privacy regression introduced by the
  privacy sprint. Rasterised bitmaps stay in memory for the same reason.
- **Probe once at startup, not per page.** If the sidecar is missing or `fra` is absent, report
  `ocr_unavailable` and fall back to the current behaviour — report the page empty, never guess. A missing
  engine degrades to Sprint 2a, which is a working product.

## F. Alternative engines and the replacement strategy

Ranked by how likely each is to be the second implementation. None is built in Sprint 2.5.

| Candidate | Case for | Why not first |
| --- | --- | --- |
| **Windows.Media.Ocr** (plus Apple Vision on macOS) | Zero installer bytes, zero licensing, fast, already on the machine | **Rejected on the cross-platform rule.** It is not one engine but two, with different accuracy, different language coverage and different failure modes on Windows and macOS, and it needs a French OCR language pack we can neither confirm nor install on her PC. Still the fallback if the Tesseract bundle proves unmaintainable, and behind the port it is one new file per platform |
| **`ocrs`** (pure Rust, RTen models) | No native dependency at all, cargo-only, cross-platform | Models are English-centric and the project is young. Unknown French accent handling is not a risk worth taking in a four-day sprint. Revisit when its French models mature |
| **PaddleOCR** | Best-in-class layout and table detection from images | Python plus a model runtime in a Windows installer. This is the Python-sidecar trade `docs/DECISIONS.md` already rejected for the tabular engine, for the same reasons |
| **OCRmyPDF** | Produces a sandwiched searchable PDF, which is genuinely nice | Python, plus Ghostscript, which is AGPL. A licence question the product does not need |
| **Tesseract via FFI** (`leptess`) | No process spawn | A vcpkg build chain on Windows, for a saving measured in milliseconds on a workload of twenty pages a week |

**The replacement strategy is the port and nothing else.** `OcrProvider` is constructed once, in
`commands.rs`, and passed down. Swapping the engine touches that construction and adds one file under
`src/ocr/`. Retrieval, chunking, the index, the gateway, the workflows and the webview are untouched by
construction, and the tests prove it because they already run against a fake provider. The stored
`ocr_engine` and `ocr_engine_version` mean a swap triggers a re-index of OCR'd documents only, exactly as
changing the embedding alias triggers a full re-index today.

## G. Exact `OcrProvider` contract

```rust
/// One local OCR engine. The only thing the application knows about OCR.
/// Implementations must not touch the network, and must not write recognised text to disk.
pub trait OcrProvider: Send + Sync {
    /// Stable engine identity, stored with every recognised chunk so a later engine change
    /// invalidates what this one produced. For example "tesseract".
    fn id(&self) -> &str;

    /// Engine build, stored alongside the id. "5.3.3".
    fn version(&self) -> &str;

    /// Whether this engine can read this image format at all. Cheap, format-only:
    /// it must not open the file, and it must never be used to judge content quality.
    fn supports(&self, format: ImageFormat) -> bool;

    /// Recognise one page. One call, one page, so provenance stays page-shaped all the way
    /// down to a citation.
    fn recognise(&self, input: &OcrInput<'_>) -> Result<OcrPage, OcrError>;
}

pub struct OcrInput<'a> {
    /// Encoded image bytes, held in memory. A rasterised PDF page, or an image file read
    /// straight off the work folder.
    pub image: &'a [u8],
    pub format: ImageFormat,
    /// Relative to the work folder, so the result carries its own locator.
    pub relative_path: &'a str,
    /// 1-indexed, matching ExtractedPage and the citation shown to her.
    pub page_number: u32,
    /// BCP 47 from settings. The engine maps it to its own vocabulary; "fr-FR" -> "fra"
    /// is a lookup table in the implementation, never a literal in the caller.
    pub locale: &'a str,
}

pub struct OcrPage {
    pub relative_path: String,
    pub page_number: u32,
    pub text: String,
    /// Mean word confidence in 0.0..=1.0 when the engine reports one. Kept internally,
    /// never rendered as a number to the user.
    pub confidence: Option<f32>,
    pub engine: String,
    pub engine_version: String,
    pub status: OcrStatus,
}

pub enum OcrStatus {
    /// Usable text, above the confidence floor.
    Recognised,
    /// The engine read the page but is not confident enough to feed retrieval.
    /// The text is kept for the record and the page is treated as empty.
    BelowThreshold,
    /// The engine ran and found no text. A blank page, or a photograph of a wall.
    NoTextFound,
}

pub enum OcrError {
    /// The engine is not installed or did not start. Degrade to Sprint 2a behaviour.
    EngineUnavailable,
    /// The engine runs but has no model for this locale.
    LanguageUnavailable { locale: String },
    /// These bytes are not a decodable image.
    UnreadableImage,
    /// The page took longer than the per-page budget.
    Timeout,
}
```

Deliberately absent, per the brief's "do not over-engineer": bounding boxes. Tesseract's TSV output already
carries them, so adding `Vec<WordBox>` to `OcrPage` later is purely additive and blocks nothing. Also
absent: a batch method. Twenty pages a week does not justify one, and a per-page call is what keeps
provenance page-shaped.

`OcrStatus::BelowThreshold` is the mechanism behind the brief's section 10. Low-confidence text does not
reach the index, so it cannot be retrieved, so it cannot become a confident sentence from the model. The
uncertainty is resolved at ingestion, where it is still measurable, rather than at generation, where it is
not.

## H. PDF text-layer detection strategy

Per page, cheapest path first, exactly as the brief's section 17 asks.

```text
for each page of the PDF
  native text from pdf_extract
    ├── >= MIN_TEXT_LAYER_CHARS after trimming   -> PageOrigin::TextLayer, no OCR, no raster
    └── otherwise                                -> rasterise at 300 dpi -> OcrProvider
                                                    ├── Recognised      -> PageOrigin::Ocr
                                                    └── anything else   -> empty page, reported
```

`MIN_TEXT_LAYER_CHARS` starts at **48**, measured against the fixtures and adjusted once. The number has to
exist because the failure it prevents is common: a scanned letter carrying a text layer that holds only the
scanner's header, or a page number, and no content. Zero would accept that page as born-digital and index
nothing useful. Purely empirical, like the formula thresholds borrowed from `LocalGridMind`.

Three details the current code does not handle and this sprint must.

- **`pdf_extract` failing is not the same as a page being empty.** `extraction.rs:75` collapses any parse
  failure into a single empty page, losing the page count. With pdfium available, a failure becomes "ask
  pdfium how many pages there are, and send every one of them to OCR", which is the correct behaviour for
  the image-only PDFs this sprint exists to read.
- **Mixed documents are the normal case**, not an edge case: a born-digital covering letter with a scanned
  result stapled behind it. Per-page detection is the whole point.
- **Rasterise only the pages that need it.** A twelve-page report with one scanned appendix costs one
  raster and one OCR call, not twelve.

## I. JPEG and PNG integration strategy

An image file is a one-page document, and that is the entire integration.

```text
discovery.rs    jpg/jpeg/png join the extension allow-list
extraction.rs   read bytes -> OcrInput { page_number: 1 } -> OcrProvider
                -> ExtractedDocument { pages: [ExtractedPage { page_number: 1, origin: Ocr }] }
```

From that point the file is indistinguishable from a PDF page: same chunking, same index, same citation
shape, same refusal when nothing was read. No image is copied, no thumbnail is written, no derived file is
created anywhere. The bytes are read, recognised in memory, and forgotten; only the text reaches SQLite,
in the same `%LOCALAPPDATA%` index that already holds document text.

A citation for an image reads as page 1 of that file, which is accurate and not misleading.

## J. Provenance integration

`docs/ARCHITECTURE.md` already defines `derivation` to separate a verified fact from a generated one, with
four variants. OCR needs a **fifth**, and it belongs there rather than in a side channel:

```text
derivation: Extracted                             copied from the file's own text layer
          | Recognised { engine, confidence }     a machine read a picture of it
          | Computed { operation, operands, rows } we calculated it
          | FormulaStored { expression }           the file says so; we did not verify it
          | ModelAsserted                          the model said it
```

`Recognised` is not `Extracted`, and collapsing the two would be the most expensive shortcut available in
this sprint. "The letter says 6.8" and "a machine thinks the letter says 6.8" are different claims, and the
difference is exactly the one the brief's section 10 is about. Keeping it as a field rather than as wording
means the interface can mark an OCR-sourced citation, and a test can assert that no chunk labelled
`Recognised` was ever presented as `Extracted`.

The chain is unchanged in shape, so no second pipeline appears:

```text
file -> page -> (text layer | OCR) -> chunk -> Evidence -> citation "file, page, passage"
```

Fabrication is prevented structurally rather than by instruction. A citation may only name a chunk that
retrieval returned; a chunk exists only if a page produced text; a page produces text only from a text
layer or from a completed `OcrStatus::Recognised`. There is no path from a failed OCR to a citation,
because a failed OCR produces no row. When every page of a document fails, the document appears in
`IndexSummary::empty_files` by name, and a question about it reaches `InsufficientEvidence` — the refusal
that already exists and already has a test.

## K. Caching strategy

**No new cache.** The existing one covers it, which is the best possible answer to the brief's "only
implement caching if it fits the existing architecture cleanly".

`indexing.rs:47` compares each file's SHA-256 against `documents.sha256` and skips unchanged files before
extraction runs. An unchanged scan is therefore never OCR'd twice, today, with no new code. The recognised
text is persisted as chunk rows, which is the cache.

One addition is worth making, and it is two columns rather than a mechanism. `documents` gains `ocr_engine`
and `ocr_engine_version`. A file is re-processed when its hash changed **or** when it was OCR'd by a
different engine or version than the one now configured. This mirrors a decision already recorded for
embeddings: changing the embedding alias forces a re-index because vectors from two models cannot be
compared. Text from two OCR engines can at least coexist, so the invalidation is narrower — only OCR'd
documents, not the whole index.

What is deliberately not built: a bitmap cache, a page-image store, a separate OCR results table. Each
would put a second copy of document content on disk, and `docs/DECISIONS.md` already says we do not
duplicate the corpus.

## L. Windows deployment implications

Sprint 4 builds the installer, so Sprint 2.5 owes it a decision that does not make Sprint 4 harder.

Read with section O: Windows is the pilot's platform, macOS is a **mandatory** target, and the table
below is about the first without excluding the second.

| Concern | Position |
| --- | --- |
| Installer size | Plus roughly 50–80 MB (Tesseract, its DLLs, `fra.traineddata`, pdfium). Irrelevant on a practice PC, and it buys reproducibility |
| Bundling mechanism | Tauri 2 `externalBin` for the executable, `resources` for the traineddata and the DLLs. Both are existing, documented `tauri.conf.json` fields — no custom install step |
| Code signing | If the application is signed, the sidecar must be too, or SmartScreen will complain about the child process. This is a Sprint 4 task and a Sprint 4 risk line, not a Sprint 2.5 one |
| Antivirus | An unsigned `.exe` spawning from `%LOCALAPPDATA%` is a classic false positive. `docs/DECISIONS.md` already lists antivirus locks as a known blind spot; add the sidecar path to whatever exclusion note the install procedure carries |
| Missing engine at runtime | Probed once at startup. `ocr_unavailable` in the interface, and the product still works as Sprint 2a. A failed OCR install must never be a failed launch |
| The Mac mini | Unaffected. OCR runs on the workstation. The server never sees a page image, and the sidecar is not in the Compose stack |
| macOS client, later | Same sidecar, a macOS build of Tesseract. If that ever looks unreasonable, Apple Vision behind the same port is one file |

## M. Tests to add

The brief's section 16, mapped onto the suites that exist. The largest single piece of work here is the
fixtures, because the current ones cannot exercise OCR at all.

**Fixtures first.** `scripts/gen-sandbox-fixtures.py` gains four outputs, all fictional:

| Fixture | Stands in for |
| --- | --- |
| `2026-03-22_courrier-rhumatologie-scan.pdf` | A real image-only PDF: French text rendered to a bitmap and embedded, no text layer. The one fixture without which nothing else in this list is a test |
| `2026-03-24_compte-rendu-mixte.pdf` | Page 1 born-digital, page 2 an image. The mixed-document case, which section H exists for |
| `2026-03-26_ordonnance-scan.jpg` and `.png` | The image-file path |
| `2026-03-28_illisible.png` | Noise. No recoverable text |
| `2026-03-20_radiographie-scan.pdf` | **Kept as is.** No image at all, so it stays the "nothing to read" case and its two existing assertions keep their meaning |

That generator is stdlib-only on purpose, and rendering text to a bitmap with the standard library alone is
not reasonable. The generator is a development tool rather than product code, so it may take a Pillow
dependency, guarded and documented in `fixtures/gp-sandbox/README.md`; the outputs are committed as
binaries, as the existing PDFs already are. Nothing is added to the product's dependencies.

**Hand-supplied scans are welcome and better, under one condition.** The owner offered to provide real
PDFs containing images, which is genuinely more valuable than generated ones: a real scanner produces
skew, speckle, JPEG artefacts and a header band that a clean synthetic bitmap never will, and those are
exactly the conditions that decide whether the recognition is usable. Two rules govern them, and neither
is negotiable.

- **Fictional content only.** `fixtures/` is committed to a Git repository, so anything placed there is
  permanent and copied to every clone. The legal gate in `docs/ROADMAP.md` is not passed: no patient
  name, no real correspondent, no genuine report, not even redacted. The safe way to produce one is to
  type invented text in Word, print it, and scan the paper — a real scan of a fake letter.
- **Check before committing, not after.** Strip metadata, confirm the file holds no embedded text layer
  from the scanner's own OCR unless that is the case being tested, and name it in the fixtures README.

If a genuine practice scan is ever needed to diagnose a recognition problem, it goes in the gitignored
`fixtures/gp-sandbox/real/` tree that already exists for this purpose and never leaves the workstation.
The generated fixtures stay regardless: they are deterministic, they regenerate from source, and a test
suite should not depend on a binary nobody can reproduce.

**Rust unit tests**, against a fake provider, so they run without Tesseract installed:

- A PDF with a text layer produces `PageOrigin::TextLayer` and the fake provider records **zero** calls.
  This is the brief's first test and it is the one that protects the CPU budget.
- An image-only PDF produces `PageOrigin::Ocr`, the recognised text, and the right page number.
- A mixed PDF calls the provider exactly once, for page 2.
- A JPEG and a PNG each produce a one-page document with page number 1.
- `OcrError::UnreadableImage` produces an empty page, the file named in `empty_files`, and no chunk.
- `OcrStatus::BelowThreshold` produces no chunk, so low-confidence text cannot be retrieved.
- A provider returning `EngineUnavailable` degrades to today's behaviour rather than failing the index.

**Integration**, extending `tests/end_to_end_retrieval.rs` with a fake provider beside the fake gateway:
a question answerable only from the scanned letter returns evidence citing that file and page; the
unreadable image still yields the refusal.

**Privacy**, the group worth writing carefully because it is the one a reviewer cannot check by reading:

- The recognised text never reaches the gateway except inside a selected excerpt. Assert on what the fake
  gateway received, not on intent.
- The gateway register holds no OCR text. Extend `apps/server/tests/test_no_store.py`; the
  `MetadataOnlyFilter` already makes this hard to break, and the test makes it visible.
- **No file is written outside the index.** Walk the temporary directory tree after an OCR run and assert
  that no new file appeared. This is the test that catches the Tesseract-writes-a-`.txt`-file mistake
  described in section E, and it must exist before the real provider is wired in.
- Source files are unmodified: hash the fixture before and after indexing.

**Regression.** The full Sprint 1 and 2a suites — 31 pytest, 47 vitest, 22-plus `cargo test`, plus
`extraction_fixtures.rs` and `end_to_end_retrieval.rs` — stay green. The two discovery and extraction
tests that assert PNG is unsupported change deliberately and are the only existing assertions this sprint
is allowed to invert.

## N. Exact Sprint 2.5 implementation sequence

Four days of work in the 20–30 September window Sprint 2a no longer needs. One branch per step, each
ending green, per `.cursor/rules/git-workflow.mdc`. The remaining slack is not spare capacity for extra
features: it belongs to the fixtures and the Windows bundle, which are the two items most likely to be
underestimated here.

**Day 1 — the contract and something to test it against.**

1. `docs/` updated: this assessment, the roadmap, the architecture port, the reversed decision. *(Done
   with this assessment.)*
2. Fixtures: the real image-only PDF, the mixed PDF, the JPEG, the PNG, the unreadable image.
3. `src/ocr/mod.rs`: the port, `OcrInput`, `OcrPage`, `OcrStatus`, `OcrError`, and a fake provider under
   `#[cfg(test)]`. No engine yet.
4. `error.rs` and both locale catalogues: the four new machine codes.

**Day 2 — the engine, isolated.**

5. `src/ocr/tesseract.rs`: sidecar discovery, the startup probe, locale mapping, stdin-and-stdout
   invocation, TSV confidence parsing, the per-page timeout.
6. `src/raster.rs`: `pdfium-render`, page count and page-to-bitmap in memory.
7. `tauri.conf.json`: `externalBin` and `resources`. Verify the bundle builds on Windows now, not in
   Sprint 4.

**Day 3 — the wiring.**

8. `extraction.rs`: per-page text-layer detection, `PageOrigin`, the parse-failure path through pdfium.
9. `discovery.rs`: the image extensions, and the two inverted tests.
10. `chunking.rs`, `index_store.rs`, `indexing.rs`: `origin` and `confidence` through to `Evidence`; the
    two `documents` columns; `ocr_files` and `low_confidence_files` in `IndexSummary`.
11. React: the OCR marker on a citation and the "read by OCR" line in the index summary, both through the
    catalogues.

**Day 4 — proof.**

12. The unit tests of section M.
13. The integration and privacy tests, the no-new-files assertion first.
14. Full regression, `ruff`, `tsc`, `cargo clippy`.
15. Measure on the fixtures: per-page OCR time, and the empty-page rate. `docs/DECISIONS.md` lists
    "measure the empty-page rate before promising intelligent filing" as a blind spot; this is where that
    number comes from.

**Acceptance, 30 September**, alongside milestone A. A scanned fictional letter answers a question with
a citation naming its file and page; a born-digital PDF indexes with zero OCR calls; an unreadable image
produces a refusal and no text; no file is written outside the index; the Sprint 1 and 2a suites are
green. If OCR slips, milestone A still stands on the Sprint 2a chain, which is why this sprint sits in
front of it rather than inside it.

## O. Cross-platform audit — Windows and macOS

Requested by the owner on 19 September 2026, who set the rule plainly: **both operating systems are
mandatory.** This section reports what was verified, what was read, and the one real gap found. It is not
a promise; it is an inspection with its limits stated.

**Verified by running it.** `cargo test --lib` on Windows: 55 passed, 0 failed. That is the current
state of the pilot's platform and it is green.

**Not verified, because it cannot be.** Nothing in this repository has ever been compiled or tested on
macOS. There is no `.github/` directory, no CI of any kind, and no Mac in the project yet — the Mac mini
on order is the *server*, and it runs the Compose stack rather than the client. So "portable to macOS" is
today an architectural intention, well implemented, and an untested one.

### What is genuinely portable, by design

| Area | Finding |
| --- | --- |
| Platform paths | `settings.rs` and `commands.rs` take every directory from `app.path()` — `app_config_dir`, `app_local_data_dir`, `home_dir`, `document_dir`. No literal path anywhere in the product code |
| The work-folder allow-list | `system_trees` and `cloud_trees` are paired `#[cfg(windows)]` / `#[cfg(not(windows))]` blocks, and the non-Windows branch is a real macOS implementation: `/System`, `/Library`, `/Applications`, plus `~/Library/CloudStorage` and `~/Library/Mobile Documents` for iCloud and every File Provider client since macOS 12.3. This is better than most projects manage |
| Case-insensitive comparison | `components_for_comparison` lowercases only under `cfg!(windows)`, which is right: macOS is case-insensitive by default but case-preserving, and lowercasing there would be wrong for a case-sensitive volume |
| Cloud placeholders | The reparse-point check is `#[cfg(windows)]` with a `#[cfg(not(windows))]` no-op. Correct, and the macOS equivalent is already covered by the `CloudStorage` and `Mobile Documents` trees |
| Relative path storage | `discovery.rs` stores forward-slash relative paths deliberately, so an index is not tied to the machine that built it |
| Dependencies | `pdf-extract`, `zip`, `quick-xml`, `walkdir`, `sha2`, `serde` are pure Rust. `reqwest` uses `rustls` rather than the platform TLS stack, explicitly so that both platforms behave identically. `rusqlite` is `bundled`, so SQLite compiles from source and needs only the Xcode command-line tools on a Mac |
| The gateway | Python and FastAPI in Docker Compose, the same file on both. Genuinely platform-neutral |
| Tesseract and pdfium | Both ship prebuilt for Windows x64, macOS arm64 and macOS x64. Section E |

### The gap

**Most of `work_folder.rs`'s tests would fail on macOS**, and they would fail for a reason that has
nothing to do with the code being wrong.

The test helper `windows_policy()` builds its paths from Windows literals — `C:\Users\practice\Documents`
and so on — and the tests then run on every platform. On Unix a backslash is an ordinary filename
character, so `C:\Users\practice\Documents` is not a Windows path at all: it is a single relative
component whose name happens to contain backslashes. `Path::is_absolute` on Unix is equivalent to
`has_root`, which is true only for a path beginning with `/`. And `check_path` tests `is_absolute` before
it reaches any allow-list rule.

So on macOS these assertions would return `work_folder_path_not_absolute` instead of what they expect:

```text
the_whole_of_documents_is_refused                                    expects work_folder_is_protected
the_home_folder_itself_is_refused                                    expects work_folder_is_protected
a_system_tree_is_refused_including_what_is_inside_it                 expects work_folder_is_protected
the_reported_sync_root_is_refused_with_everything_inside_it          expects work_folder_is_cloud_synced
the_redirected_documents_folder_is_refused_as_synchronised           expects work_folder_is_cloud_synced
a_sync_folder_is_refused_by_name_even_when_the_platform_reported_no… expects work_folder_is_cloud_synced
a_folder_that_merely_looks_like_a_sync_name_is_accepted              expects Ok
the_suggested_folder_sits_in_the_home_and_is_accepted                expects Ok
```

The *policy* is portable; its *tests* are not. That distinction matters, because it means this is a
half-day of work rather than a redesign: give the helper a platform-appropriate root, the way
`a_drive_root_is_refused` already does with `if cfg!(windows) { "C:\\" } else { "/" }`, and the same
assertions then check the same rules on both systems.

Two smaller items in the same family. `discovery.rs`'s symlink-refusal test is `#[cfg(unix)]`, so the one
rule that stops a link escaping the work folder is **never exercised on the pilot's own platform**;
Windows can create a directory junction without elevation, which would cover it. And `tauri.conf.json`
declares no `bundle.targets` and no `.icns`, which is fine for a Windows build today and will need a line
when a Mac build is first produced.

### What Sprint 2.5 must do about it

1. **Write no `#[cfg(windows)]` in `src/ocr/`.** Platform differences belong in the bundler configuration
   and in sidecar discovery, never in the recognition path.
2. **Parameterise the `work_folder.rs` test paths** so the suite is honest on both systems. This is
   pre-existing debt, not OCR work, and it takes its own branch.
3. **Add a CI workflow that builds and tests on `windows-latest` and `macos-latest`.** Until that exists,
   "compatible with both" is an opinion. It is the cheapest single thing that would make the claim true,
   and it is worth doing before the OCR code that would otherwise drift.

Item 3 is the honest answer to the question. The architecture is portable and was clearly written by
someone thinking about both platforms; nothing has ever proved it.

---

## Decisions taken

All confirmed by the owner on 19 September 2026 and recorded in `docs/DECISIONS.md`.

1. **The first OCR engine is Tesseract 5 as a bundled sidecar**, with pdfium for rasterisation. Confirmed,
   and the stated reason is binding beyond this sprint: **Windows and macOS are both mandatory**, so an
   engine that exists on only one of them is disqualified however good it is. No `#[cfg(windows)]` in
   `src/ocr/`.
2. **OCR is ingestion.** It lives beside extraction on the workstation, behind `OcrProvider`, and no GP
   workflow may contain OCR code.
3. **`Recognised` is a fifth `derivation` variant**, not a flavour of `Extracted`. What a machine read is
   structurally distinct from what a file states.
4. **Low-confidence OCR does not reach the index.** Uncertainty is resolved at ingestion, where it is
   measurable.
5. **Tabular data is confirmed as sprint 2b, after sprint 4, and it is not optional.** The pilot GP has no
   spreadsheets, so it earns nothing for milestone B and stays out of the pilot window. It is not
   speculative work either: the owner identifies hospital staff who schedule from exactly these files as a
   real user group. It ships after the pilot holds, built to the standard in
   `docs/SPRINT-2-ASSESSMENT.md`, and a later sprint may not quietly drop it or reduce it to "spreadsheets
   as text chunks".
6. **A cross-platform CI job comes before the OCR code.** Section O found that nothing has ever been built
   on macOS, and that most of the work-folder tests would fail there for a test-fixture reason rather than
   a design one. Both are cheap to fix and expensive to discover later.
