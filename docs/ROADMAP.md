# Roadmap — v0 sprint

**Revised 16 September 2026.** Replaces the earlier ten-phase plan (mapping table at the end).
Canonical direction: `docs/BRIEF-V0-PROTOTYPE.md`. Sprint rule: `.cursor/rules/v0-sprint.mdc`.

**Amended 18 September 2026** after the Sprint 2 architectural reframe: Sprint 2 splits into 2a
(documents, carries milestone A) and 2b (tabular data). Reasoning and the full assessment:
`docs/SPRINT-2-ASSESSMENT.md`. Long-term direction, which is deliberately **not** in this file:
`docs/PLATFORM-VISION.md`.

**Amended 19 September 2026.** Sprint 2a delivered eleven days early, and scan OCR moves into v0 as
**Sprint 2.5**, between the document chain and the GP workflows. Direction:
`docs/BRIEF-SPRINT-2.5-OCR.md`. Engineering assessment: `docs/SPRINT-2.5-ASSESSMENT.md`. The decision it
reverses ("scan OCR out of v0") is updated in `docs/DECISIONS.md`.

**Amended 23 September 2026.** **Sprint 2a.5** is inserted as hardening between sprint 2.5 and sprint 2b.
It adds no ingestion format and no user-facing feature: it makes the work folder a deterministic,
authoritative source of filesystem facts, so that a file count, a file name or an extension can no longer
be inferred by a model from retrieved passages. Design: `docs/WORK-FOLDER-INVENTORY.md`. Nothing else moves;
sprint 2b keeps its date.

One goal: an **installable** prototype the pilot GP can use, which answers **with its sources** on her own
documents. Given a choice between one more feature and a more reliable, private, testable and replaceable
flow, take the second.

The chain to make hold end to end:

```text
She opens Assistant Cabinet AI
  → she picks a folder
  → the documents are indexed on her workstation
  → a scan or a photo is read locally by OCR, like any other document
  → she asks a question
  → retrieval happens on her workstation
  → only the useful excerpts go to the gateway
  → the local model answers
  → the answer cites file, page and passage
  → no document is kept on the server
```

## Two milestones, not one

The brief says four weeks; "end of the month" leaves two. So the same technical chain is cut into two
milestones.

| Milestone | Date | What must work | On what |
| --- | --- | --- | --- |
| **A — the chain holds** | **30 September 2026** | Native window, gateway, local index, answer with sources, isolation test | `fixtures/gp-sandbox/` (fictional) |
| **B — she uses it** | **14 October 2026** | The three GP flows, the Windows installer, deployment at the practice | Her own documents, **after** the legal gate |

## Sprint order

Ingestion capability order, which is what the code depends on:

```text
Sprint 2a  documents      PDF / DOCX / TXT / MD
   ↓
Sprint 2.5 local OCR      image-only PDF / JPEG / PNG
   ↓
Sprint 2a.5 work folder   inventory, file identity, reference resolution (no new format)
   ↓
Sprint 2b  tabular data   CSV / XLSX
   ↓
Sprint 3   GP workflows   summary, naming with duplicates, structured extraction
   ↓
Sprint 4   deployment     Windows installer, the practice
```

Calendar order, which differs in one place on purpose:

| Sprint | 1 | 2a | 2.5 | 2a.5 | 3 | 4 | 2b |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Dates | delivered 17 Sep | delivered 19 Sep | delivered 21 Sep | 23 Sep | 1–7 Oct | 8–14 Oct | from 15 Oct |

Sprint 2b is numbered before Sprint 3 and dated after Sprint 4. The reason was recorded on 18 September
and has not changed: `docs/PILOT-GP.md` finds no spreadsheet anywhere in her measured workflow, so tabular
work earns nothing for milestone B, and placing it earlier would displace either her three workflows or
the deployment.

Late is not the same as optional. The owner confirmed on 19 September that hospital staff who schedule
from `.csv` and `.xlsx` are a named user group, so sprint 2b is a committed capability with a deliberately
late date, not a maybe. What it must not become is a spreadsheet application; the nine named operations
and the data-grid-sheets-only gate exist for that.

OCR is the opposite case, which is why it moved into v0: she receives 10 to 20 scans a week today, and
Sprint 3 cannot summarise, classify or name a document the pipeline cannot read. It takes the window
Sprint 2a vacated by finishing eleven days early, and it costs no other sprint a single day.

## Sprint 1 — vertical slice (16 → 23 September) — **delivered 17 September 2026**

Deliverable: `Tauri → FastAPI → Ollama → answer`, visible in a window with no address bar.

- `apps/desktop`: Tauri 2 window, React, TypeScript. Title and icon **Assistant Cabinet AI**. Light, dark
  and follow-the-system themes. Language setting alongside the theme (`docs/LANGUAGE-AND-LOCALE.md`).
- `apps/server`: FastAPI and Pydantic. `/health`, `/v1/chat/completions` (OpenAI-compatible, streaming),
  an `AIProvider` interface with Ollama as the first implementation.
- Settings on both sides: server URL, model alias, work folder, locale. No constants in code.
- Pick the work folder through the system dialog, and refuse the drive root, system folders, and the whole
  of Documents.
- `compose.yaml`: add the `server` service and point Open WebUI at the gateway instead of Ollama.
- Acceptance: she types a sentence and gets a French answer; with the server stopped she gets a clear
  message rather than a stack trace.

**Delivered.** Merged as pull request #1, `feat: vertical slice from tauri window to ollama through the
gateway`, 86 files. What exists now, verified on the pilot workstation:

- `apps/server`: `GET /health`, `GET /v1/models`, `POST /v1/chat/completions` (streaming and not),
  `AIProvider` with `OllamaProvider` as the only implementation, alias resolution from configuration,
  locale packs, the output-language directive appended to an English body, no-store headers, and a
  metadata-only register. 31 pytest, `ruff` clean.
- `apps/desktop`: React and TypeScript window with themes, settings, chat, server status and error
  banner; every visible string through the `en-US` and `fr-FR` catalogues. 47 vitest, `tsc` clean.
- `apps/desktop/src-tauri`: `settings.json` in the app-config directory, the work folder allow-list, the
  gateway client with SSE parsing, and five Tauri commands as the only bridge to the webview.
  22 `cargo test`.
- `compose.yaml`: the `server` service on `127.0.0.1`, with Open WebUI routed through the gateway.
- Evidence: `/health` green with the provider reachable, a French answer produced from the English system
  prompt, streaming delta by delta, and a register line carrying hashes and counts but no document text.

Not done in sprint 1, and inherited by sprint 2: there is no `/v1/embeddings` route although
`AIProvider.embed` exists, `fixtures/gp-sandbox/` still holds only `.txt`, there is no `uv.lock`, and
nothing reads a document yet.

## Sprint 2a — local document retrieval (24 → 30 September) — **delivered 19 September 2026**

Deliverable: the chain above, on the fictional files, with citations. This sprint carries milestone A,
so nothing else may be added to it.

- Extraction on the workstation: native PDF, DOCX, TXT, MD. Chunking that **keeps** file, page and section.
- File discovery over the work folder, extension-driven, symbolic links refused. The same mechanism must
  later serve a legal or accounting practice: no GP-specific loader, no assumption about folder contents.
- Local index (SQLite: full-text search plus vectors) in `%LOCALAPPDATA%` (`app_local_data_dir()`), behind
  a replaceable interface. Not the roaming `%APPDATA%` that holds `settings.json`: the index contains
  document text, and a roaming profile copies `%APPDATA%` to a server. Neither the database nor the
  embedding model may show through into business code.
- Embeddings: first through a **no-store** call to the gateway. `POST /v1/embeddings` is done (18
  September): its own alias out of the same allow-list, caps on batch count and total characters, and a
  refusal when the runtime returns a vector count that does not match the inputs, because a silently
  short batch would bind each chunk to the wrong vector and every later citation would point at the
  wrong passage. A local ONNX computation stays possible later behind the same interface, without
  touching anything else.
- Send only the selected excerpts, with a size cap. Never the whole folder "just in case".
- An answer that cites file, page and passage, through the common `Source` model in
  `docs/ARCHITECTURE.md`. Without sufficient excerpts, the product says it did not find enough
  information rather than generating one.
- **Tests:** retrieval, source attribution, refusal to answer beyond the sources, and a test that
  **proves** the server holds no file, no chunk and no text in a database or a log after a request.
- Extend the sandbox: it currently holds only `.txt`. It needs fictional native-text PDFs, a DOCX, and one
  scanned image PDF reserved for the OCR contract.

**Delivered.** Merged as `c537805`, `feat: local document extraction, index, retrieval and sourced chat
UI`. What exists now: `discovery.rs` (extension allow-list, symlinks refused), `extraction.rs` (native
PDF, DOCX, TXT, MD), `chunking.rs` (file, page, section preserved), `index_store.rs` (SQLite, FTS5 plus
stored vectors in `%LOCALAPPDATA%`), `retrieval.rs` (lexical plus cosine, capped at 6 chunks and 6000
characters), `ask_with_sources` refusing on insufficient evidence, and `POST /v1/embeddings` on the
gateway. `tests/end_to_end_retrieval.rs` is the milestone A acceptance, automated.

Inherited by Sprint 2.5: emptiness is decided per document rather than per page, so a mixed
born-digital-plus-scan PDF loses its scanned pages silently; and the scanned-PDF fixture contains no
image, so it cannot exercise OCR. Both are detailed in `docs/SPRINT-2.5-ASSESSMENT.md`.

**Milestone A acceptance, 30 September:** on a folder of fictional files, three questions give three
sourced answers, a fourth off-topic question gets the refusal, and the isolation tests pass. **Already
met** by `tests/end_to_end_retrieval.rs`, which automates exactly that. Sprint 2.5 adds a fourth sourced
answer, from a scanned letter; if OCR slips, milestone A still stands on the chain above.

## Sprint 2.5 — local OCR (20 → 30 September)

Deliverable: a scanned document is as usable as a born-digital one. Direction:
`docs/BRIEF-SPRINT-2.5-OCR.md`. How it is built, and why each choice: `docs/SPRINT-2.5-ASSESSMENT.md`.

**Delivered, 20–21 September, ahead of the 30 September window.** `OcrProvider`/`TesseractProvider`
wired into `extraction.rs` per section N; `cargo test --lib` green (80 tests) including the
zero-OCR-calls-on-a-text-layer and no-file-written-outside-the-index cases from section M. Two real
bugs found and fixed on the way, both logged in `docs/TROUBLESHOOTING.md`: sidecar discovery missing
`tauri dev`'s executable location, and Tesseract's bare `tsv` argument being a config file we never
bundled rather than an output-format flag. **Acceptance confirmed by human re-test** on
`fixtures/gp-sandbox/`: the scanned rheumatology letter and prescription answer with a citation
naming file and page, the index summary went from 5 unreadable files to 0, and a born-digital PDF
still costs zero OCR calls. The measured empty-page rate on the fixtures is 0% after the fix.

It takes the window Sprint 2a vacated. `docs/PILOT-GP.md` records about 10 paper letters and 10 to 20
scans per week, plus organised-screening second readings that arrive on paper. Without OCR those
documents reach the GP workflows as an empty extraction and a refusal, and Sprint 3 cannot summarise,
classify or name a document the pipeline cannot read.

The core is four days of work; the window is eleven. The slack goes to the two things this sprint is
most likely to underestimate — generating fixtures that genuinely exercise OCR, and proving the engine
bundles into a Windows build — and to milestone A hardening.

- **OCR is ingestion, not a GP workflow.** It sits beside extraction on the workstation. No Sprint 3
  workflow may contain OCR code; they consume the normalised document the pipeline produces.
- **One port, `OcrProvider`, one implementation.** Business code names no engine. The first is Tesseract 5
  as a bundled sidecar with pdfium for rasterisation, confirmed 19 September; swapping it touches one
  constructor and one file, and changes nothing in retrieval, chunking, the index, the gateway or the
  webview.
- **Windows and macOS, from the same source.** Both are mandatory, which is what chose Tesseract over the
  platform OCR engines: those are two engines with two accuracies and two failure modes. No
  `#[cfg(windows)]` in `src/ocr/`; the only platform-specific part is which prebuilt binary the bundler
  picks up. A CI job that builds and tests on both comes **before** the OCR code, because nothing in this
  repository has ever been compiled on macOS.
- **Local only, in memory.** No cloud OCR, no external document service, no Internet requirement, and no
  recognised text written to any file outside the index — which rules out the OCR engine's default
  "write the result next to the input" behaviour. A test asserts no new file appears.
- **Cheapest path first, per page.** A page whose text layer is usable is never rasterised and never
  OCR'd. A mixed document costs one OCR call per scanned page, not one per page.
- **Inputs:** image-only PDF, JPEG, PNG. `.tif` is one string in the allow-list when a scanner needs it.
- **Provenance:** a fifth `derivation` variant, `Recognised { engine, confidence }`, distinct from
  `Extracted`. What a machine read is not what a file states, and the interface says so on the citation.
- **Uncertainty is resolved at ingestion.** Low-confidence pages do not enter the index, so they cannot
  be retrieved and cannot become a confident sentence. Confidence is kept internally; no score on screen.
- **No new cache.** The existing SHA-256 skip in `indexing.rs` already prevents re-reading an unchanged
  scan. Two columns, `ocr_engine` and `ocr_engine_version`, make an engine change invalidate only the
  documents that engine produced.
- **Degradation is a feature.** A missing or broken engine reports `ocr_unavailable` and the product
  falls back to Sprint 2a behaviour. It never fails to launch, and it never guesses.
- **Deployment is a selection criterion, not an afterthought.** The bundle is verified to build on
  Windows during this sprint, not discovered in Sprint 4. Signing the sidecar is a Sprint 4 task.
- **Tests:** a native PDF invokes OCR zero times; a scanned PDF answers with file and page; JPEG and PNG
  reach retrieval; an unreadable image produces a refusal and no fabricated text; nothing is written
  outside the index; the gateway register holds no recognised text; Sprint 1 and 2a stay green.
- **Fixtures:** a genuine image-only PDF, a mixed PDF, a JPEG, a PNG and an unreadable image. The current
  `2026-03-20_radiographie-scan.pdf` has no image in it at all and stays as the "nothing to read" case.
  Hand-made scans are welcome and more realistic than generated ones, **fictional content only**: print
  an invented letter and scan the paper. `fixtures/` is committed and permanent, and the legal gate is
  not passed.

**Out of this sprint, deliberately:** DOCX to PDF and any other document conversion, layout-aware OCR,
tables detected from images, handwriting, GPU acceleration, camera capture, a second engine.

**Acceptance, 30 September:** a fictional scanned letter answers a question with a citation naming its
file and page; a born-digital PDF indexes with zero OCR calls; an unreadable image produces the refusal;
no file is written outside the index; the measured empty-page rate on the fixtures is written down.

## Sprint 2a.5 - work folder intelligence (23 September)

Deliverable: the application knows exactly what is in the work folder, and says so without asking a model.
Design and non-goals: `docs/WORK-FOLDER-INVENTORY.md`.

Hardening, not a feature. It was added because the chain had no authoritative answer to "what is in my
folder?": the only thing that knew about files was retrieval, which returns passages, so a folder of
fifteen files could be reported as six, an ambiguous file name could be resolved silently, and a document
claiming to be a PDF could change what the product said its extension was.

- Three contracts, deliberately generic so sprint 2b reuses them rather than redesigning them:
  `FileRecord` (which file is this), `WorkFolderInventory` (what is in the folder), and
  `FileReferenceResolver` (which file did she mean). Identity is the existing content SHA-256, so
  `Source.origin.sha256` keeps its meaning and no second hashing model appears.
- Filesystem facts are answered from `std::fs` plus the local index, never from a retrieved chunk and never
  by a model. Counting, listing, listing by extension and showing the folder structure make **zero** gateway
  calls, and still answer with the server stopped.
- The question vocabulary is a locale pattern pack loaded as data, as sprint 2b's will be
  (`docs/DECISIONS.md`). Rust returns machine codes; the React catalogues write the sentence.
- An ambiguous reference is a result, not a tie to break. Two files named `neurologie.pdf` produce a
  question, never a choice.
- An explicitly named file constrains retrieval to that file, which removes cross-document contamination
  from the most common kind of question she asks.
- A question about **every** document ("summarise each document") is answered from every indexed file rather
  than from the six that ranked highest, because the inventory supplies the list instead of the ranker. The
  answer states how much of the folder it rests on, computed in Rust so the model cannot omit it. Found on
  the pilot workstation on 23 September, when a folder of 15 files with 10 indexed was summarised from 6.
- **Tests:** `tests/work_folder_inventory.rs`, including adversarial fixtures whose content contradicts the
  filesystem, and a gateway double that fails the test if a deterministic question reaches it.

**Out of this sprint, deliberately:** any CSV or XLSX processing, workbook inventory, cross-domain
reasoning, semantic file resolution, background folder watching, and any file-management interface beyond
showing the counts.

**Delivered, 23 September 2026.** Merged as `6c78371`, `feat: make the work folder an authoritative source
of filesystem facts` (pull request #11). What exists now: `file_record.rs` (`FileRecord`, `FileKind`,
`Readability`, `ProcessingStatus`), `inventory.rs` (`WorkFolderInventory`, deterministic ordering, joined
onto the local index), `file_reference.rs` (`FileReferenceResolver`, exact/ambiguous/no-match resolution),
`folder_questions.rs` (deterministic routing) and `work_folder_context.rs` (the compact context block sent
to the model). 152 `cargo test --lib`, 36 further integration tests across five files, 184 `vitest`, `tsc`
and `cargo clippy` clean.

Testing the same day against the pilot's own workstation folder surfaced three problems the design had not
anticipated, fixed in the same pull request rather than deferred. A total request timeout that discarded a
slow model's answer mid-stream, replaced by an inactivity bound (a setting, not a constant) that keeps a
partial answer exactly as a stop already did. A routing rule that required every word of a question to be
in the pattern pack, which failed on ordinary polite phrasing ("could you give me a list of the available
documents") and sent the question to a model that then mistranscribed the file list by hand; replaced by
intent plus subject, no content word ("summary", "mention"), and no unknown word that the indexed documents
themselves contain, checked against the existing FTS index through a `CorpusWords` port rather than a
maintained dictionary. And a resolver that matched only a whole file name, which failed on a shortened
reference such as a date dropped from a file name; fixed with suffix matching under the same ambiguity
guarantee. The interface also now tells a *file* (anything on disk) apart from a *document* (something that
could be read), labels a deterministic answer as such, and lets the existing regenerate control ask the
model instead of recomputing an answer a model never wrote in the first place — no new control added.

**Not independently verified:** model independence (`docs/ARCHITECTURE.md`) is proved structurally —
`route()` takes no model alias and no gateway — rather than by running two live aliases against a live
gateway on the same folder. And every test above ran on Windows only; there is still no macOS CI job, so
cross-platform behaviour rests on there being no `#[cfg]` in the new code, per `docs/SPRINT-2.5-ASSESSMENT.md`
section O.

## Sprint 2a.6 - a work folder she can trust between passes (24 September)

Deliverable: adding, replacing and removing a document in the work folder is an ordinary gesture that the
application notices and reports honestly, and OCR does not quietly stop working halfway through a session.

Hardening again, and found the same way sprint 2a.5's three problems were: by the pilot using her own
documents. She dropped a real scanned prescription into the work folder, analysed, and the file came back
unreadable. The only thing that made it readable again was closing the application, deleting
`%LOCALAPPDATA%\com.assistantcabinetai.desktop` and indexing the whole folder from scratch - which is not
a workflow anyone can be asked to follow twice.

- **Only the first analysis pass of a session could read a scanned PDF.** `pdfium-render` keeps its
  library bindings in a process-global cell, so `Pdfium::bind_to_library` fails on every call after the
  first; `index_work_folder` built a fresh `Rasterizer` per pass and `.ok()` swallowed the failure.
  Measured directly against the bundled library: `first ok? true  second ok? false`. The document itself
  was never at fault - driven by hand through the same bundled binaries it gave 292 words at 90.5 % mean
  confidence in 2.9 s. Fixed with one rasteriser per process (`raster::shared`) and `Rasterizer::new` made
  private so it cannot be rebuilt. Full account in `docs/TROUBLESHOOTING.md`.
- **A missing capability now says so.** `ocr_engine = NULL` in the index meant both "born-digital PDF, OCR
  not needed" and "OCR never ran", which is why nothing on screen could tell a missing engine from an
  illegible scan. `IndexSummary.unavailable_capabilities` carries the machine codes `ocrEngine` and
  `pageRasterizer`; the React catalogues write the sentence, as everywhere else.
- **Tests:** `tests/repeated_indexing.rs` - a scan added after the first pass is read by the second, a
  fourth pass still reads scans, a missing rasteriser is reported rather than blamed on the document, and a
  scan stored empty before the engine existed is retried once the engine is back, so the index never has to
  be deleted by hand. `the_shared_rasterizer_answers_every_pass_not_only_the_first` (`src/raster.rs`) asks
  the real bundled pdfium three times; `#[ignore]`d like the Tesseract sidecar test because the library is
  gitignored, and deliberately the only test in the process allowed to build a rasteriser.

The same session then closed the gap the incident exposed: the index and the folder could disagree, and
nothing in the interface said so.

- **A document removed from the folder stops being citable.** `indexing::run` dropped rows for files that
  no longer exist, and until it did, their rows outlived them - the panel stopped listing a deleted file,
  because that reads the filesystem, while retrieval went on offering its passages as evidence. The pass
  reports what it forgot, by name.
- **The panel notices a document added behind the window.** The inventory is rebuilt when the window
  regains focus, which is the moment she comes back from Explorer or Finder having dropped a file in. A
  filesystem watcher was considered and refused for v0: a background thread, debouncing a scanner writing
  a file in pieces, and a cloud-sync folder churning underneath it, for nothing more than the same answer
  a few seconds sooner (`docs/DECISIONS.md`).
- **Rebuilding the panel became cheap enough to do often.** It decides "indexed" against "changed since"
  by comparing content hashes, which meant reading every byte in the folder each time. `FileHashCache`
  reuses a hash while a file's size *and* modification time both hold. The pass itself still hashes and
  remains the only thing that decides what is stored, so what this can get briefly wrong is a hint, never
  an index.
- **The Analyse button carries the count**, and an answer says how many documents it could not have used.
  Retrieval already refuses when it has too little evidence; `AskAnswer.unanalysed_files` is the other
  half, for when there was plenty of evidence and the file she had in mind was simply not among it.
- **Reset**, beside the folder controls, empties the index behind a confirmation and leaves every document
  where it is. It exists because the only way back to "nothing analysed" was deleting `%LOCALAPPDATA%` by
  hand. The confirmation is built rather than taken from the operating system: a native one carries the
  system's words in the system's language, which is not necessarily hers.
- **See / Voir** opens the work folder in the file manager she already uses. No path crosses the bridge -
  the command takes no argument and reads the folder from settings, so the allow-list holds because there
  is nothing to allow-list.
- **Réinitialiser**, in the settings panel, puts every setting back to a first launch, the documents
  folder included, behind the same confirmation. The index is not touched, and the confirmation says so:
  choosing the same folder again finds every document still analysed.
- **Cross-platform by construction.** `reveal.rs` is the only file that names a file manager, with paired
  `#[cfg]` arms for Windows and macOS side by side and a third that returns a machine code rather than
  guessing. Nothing above it learns which platform it is on, and no capability - extraction, OCR,
  retrieval - gained a `#[cfg]` because of it (`docs/SPRINT-2.5-ASSESSMENT.md` section O).

**Out of this sprint, deliberately:** filesystem watching, automatic analysis (a pass sends chunk text to
the gateway for embeddings and can take minutes on scans - it stays an explicit, visible act), undoing a
reset, and any per-file control beyond opening the folder.

**Delivered, 24 September 2026.** `fix(desktop): bind pdfium once per process so every analysis pass reads
scanned PDFs`, then the work folder controls above. 170 `cargo test --lib`, 41 integration tests across
six files, 201 `vitest`, `tsc` and `cargo clippy` clean.

**Not independently verified:** every test ran on Windows. The macOS arm of `reveal.rs` compiles under
`#[cfg]` review only - there is still no macOS CI job, so `open` opening Finder on the work folder is
unproven until the Mac mini is on the desk.

## Sprint 3 — GP workflows (1 → 7 October)

Deliverable: the actions that cost her 1.5 to 2 hours a day. Three flows, not five.

The three consume the **normalised document** the ingestion pipeline produces, and none of them knows how
it was produced. A workflow that asked whether its input was a born-digital PDF or a scan would be a
workflow that has to change again for the next format. Do not start this sprint until Sprint 2.5 has
landed a reliable OCR path.

1. **Report summary** — reuse `prompts/gp-letter-summary.md`, in a "retrieved context" form rather than a
   "pasted text" one. Visit date, treatment changes and lab targets **according to the letter**, with citations.
2. **Naming plan with duplicates** — reuse `prompts/gp-inbox-classify.md`. Name plus month/year convention.
   Duplicates within the batch **and** against a **local** fingerprint register of files already seen.
3. **Structured extraction** — the fields she retypes by hand, as JSON validated by Pydantic, reviewed on screen.

Locks in code, never in a prompt: dry run by default, manifest, hashes before and after, file-count cap,
refusal of any path outside the work folder, refusal of symbolic links, dedicated trash folder, **no**
permanent delete, and undo for the batch. Extracted text is data and never an instruction: a test proves
it with a hostile PDF.

Nothing is transmitted. "Export" means a renamed file in the work folder, or a summary on the clipboard.
She sends it from Medilink and MSSanté as she does today.

## Sprint 4 — real deployment (8 → 14 October)

Deliverable: the software installs and runs at her practice.

- Windows installer, signed if possible, otherwise with a written install procedure.
- Failure handling: server unreachable, model missing, file locked by Word or the antivirus, disk full.
- Logging **without document content**: request id, timestamp, model, chunk count, hashes, decision. Never
  an excerpt, not even in debug mode.
- Offline and LAN testing: Mac mini at the practice, workstation on the same wired network, only the
  gateway exposed, Ollama on the mini's `127.0.0.1`.
- Security checks: path allow-list, context cap, per-person access key, no clear text on the network.
- `docs/user/`: installation and getting started, **in French**, written for her.

## Sprint 2b — tabular data (from 15 October, after milestone B)

Deliverable: `.csv` and `.xlsx` as first-class sources, with **deterministic** factual lookup that works
with the model switched off.

**Confirmed on 19 September 2026, and it is not optional.** Scheduling it late is a statement about the
pilot, not about the feature. The pilot GP has no spreadsheets, so this earns nothing for milestone B; but
hospital staff who build schedules from exactly these files are a named user group, and the capability is
the reason `docs/ARCHITECTURE.md` keeps a second pipeline rather than flattening a workbook into text.
A later sprint may not quietly drop it, defer it indefinitely, or reduce it to "spreadsheets as chunks".
It ships to the standard in `docs/SPRINT-2-ASSESSMENT.md`, or it does not ship.

Scheduled here, and not before, for one reason: `docs/PILOT-GP.md` records about twenty specialist and
imaging reports a day plus ten to twenty lab results, almost all PDF, and accounting left the scope in
September 2026 when the accountant's software took it over. There is no spreadsheet anywhere in her
measured workflow. Moving this sprint earlier displaces either her three workflows (sprint 3) or the
deployment (sprint 4). It is platform work for the next profession — the case lists and exports in
`docs/DISCOVERY.md` — and it is scheduled as such. If discovery produces an earlier need, move it then.

- `TabularDataSource` with a CSV adapter first: delimiter and encoding detection, French formats
  (semicolon, cp1252 and UTF-8 BOM, decimal comma, `dd/mm/yyyy`), a full parse rather than a sample.
- Workbook inventory persisted locally, keyed by relative path and SHA-256: sheets, dimensions, columns,
  inferred types, row counts, header row, date and numeric and categorical columns, formula presence,
  supported capabilities. No copy of the source file.
- Deterministic operations: count, distinct, sum, min, max, group sum, largest single row, filter, sort.
  A group **total** and the largest **single row** are different facts and are labelled as such.
- Question classification from a **locale pattern pack** (data, not Rust literals). The operations are
  language-neutral; the sentence comes from the React catalogues. Changing the locale cannot change a
  number.
- When the engine cannot establish an answer it returns `NOT_DETERMINISTICALLY_ANSWERABLE` with the
  available columns, and says so. It never guesses a spreadsheet value.
- XLSX through `calamine`, **data-grid sheets only**. A formula-heavy workbook is inventoried, disclosed
  and refused for factual answers. No formula evaluation, no Excel engine.
- Escalation to the model only when the deterministic path cannot answer, carrying the aggregate card as
  structured evidence — never rows. The model's citations are verified against that evidence afterwards.
- **Tests:** deterministic answers perform zero gateway calls, proved with a provider double that panics
  when called; every deterministic question still answers with the gateway stopped; the same question
  returns the same number under a different model alias.

Design, and what is reused from `LocalGridMind` versus what is deliberately not:
`docs/SPRINT-2-ASSESSMENT.md`.

## Parallel track — the legal gate (start now)

Without these four lines, sprint 4 deploys on **fictional** files and nothing else. This is not end-of-project
paperwork: it is the entry condition for milestone B.

- [ ] Written DPIA draft (purpose: administrative assistance; data; retention; risks; measures).
- [ ] Named data controller — that is **her**. To be written down, along with the project owner's role.
- [ ] **Do not overlook:** a Mac mini owned by the project owner, installed at the practice, processing
      health data, puts the project owner in a processor position. The DPIA draft must settle who owns the
      machine, who administers it, and what it retains.
- [ ] Disk encryption: BitLocker on the workstation, FileVault on the Mac mini.
- [ ] **Check OneDrive on her machine.** She saves everything to My Documents at the root. If OneDrive
      Backup is on, her patient reports are already in Microsoft's cloud, before this software exists.
      That is a finding for the DPIA draft, and she is the controller. See `docs/PRIVACY-AND-SECURITY.md`.

Detail: `docs/PRIVACY-AND-SECURITY.md`.

## Parallel track — hardware

Order the configuration in `docs/HARDWARE.md` this week (M5 Pro 18-core, 64 GB, 1 TB). It is a custom
configuration: built then shipped, available from 22 September. The plan assumes it arrives **late September**.

If it slips, sprints 1 to 3 run against the development PC over the LAN; only sprint 4 waits for the machine.

## Risks that could miss the date

| Risk | Mitigation |
| --- | --- |
| Mac mini delivered after 8 October | Sprints 1 to 3 on the development PC; installer tested on an equivalent Windows machine |
| No DPIA draft by 8 October | Sprint 4 deploys on fictional files; real documents wait |
| OCR quality on her real scans is worse than on the fixtures | Measure the empty-page rate at the end of sprint 2.5. The engine is a port: a poor result is an engine change, not a redesign |
| The OCR sidecar breaks the Windows installer | The bundle is built and run on Windows during sprint 2.5, not in sprint 4. A missing engine degrades to sprint 2a behaviour rather than failing to launch |
| macOS portability is assumed rather than proved | Nothing here has ever been compiled on macOS and there is no CI. Add a `windows-latest` plus `macos-latest` job before the OCR code, and parameterise the work-folder test paths, which today would fail on macOS for a fixture reason rather than a design one (`docs/SPRINT-2.5-ASSESSMENT.md` section O) |
| The 2019 PC cannot keep up with indexing | Batch cap, index per work folder rather than per disk, timing measured at first run |
| French output quality | The model alias is a setting; change the alias, not the code |
| Drift onto Open WebUI, voice, certificates | Frozen by `.cursor/rules/v0-sprint.mdc` |
| Tabular work pulled into milestone A | Sprint 2a is documents only. Sprint 2b is dated after milestone B, and the pilot has no spreadsheets |
| A spreadsheet application grows out of sprint 2b | Nine named operations, data-grid sheets only, no formula evaluation. A tenth operation is a decision, not a configuration |

## Mapping from the old phases

| Old phase | Becomes |
| --- | --- |
| 0 Framing | Done, except the legal gate, now a parallel track |
| 1 Compose foundation | Done; the `server` service joins it in sprint 1 |
| 2 Prompts and sandbox | Done; the `prompts/` bodies are reused in sprint 3 |
| 3 Native shell | Sprint 1 |
| 4 File action plans | Sprint 3 |
| 5 Document extraction | Sprint 2a (native text), then sprint 2.5 (scan OCR) |
| 6 Connect the window to the gateway | Sprint 1 |
| 7 Retrieval on the client | Sprint 2a — **moved up**, it carries milestone A |
| — (new) Tabular data | Sprint 2b, after milestone B |
| 8 Mac mini port | Sprint 4, at the practice |
| 9 Field pilot | After 14 October |
| 10 More professions and agents | Unchanged: after real validation |

## After 14 October, not before

Sprint 2b (tabular data) comes first, then voice, vision, mobile, app stores, a second profession,
autonomous agents, fine-tuning, an accounting module, the Ameli account, automatic transmission, RBAC,
billing, analytics.

Scan OCR left this list on 19 September 2026 and became sprint 2.5. What stays out of v0 is everything
*around* it: layout-aware OCR, tables read from images, handwriting, GPU acceleration, camera capture,
batch OCR tooling and a second engine.

Where that ordering is heading, without dates: `docs/PLATFORM-VISION.md`.
