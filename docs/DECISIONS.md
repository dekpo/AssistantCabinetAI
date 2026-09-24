# Frozen decisions

One place to check before proposing something that was already settled. Consolidates the decision tables
that were spread across the earlier French notes. If a decision changes, change it here and say why.

## Product and scope

| Subject | Decision |
| --- | --- |
| Practice screen | A native Tauri window named **Assistant Cabinet AI**, no URL. Not Open WebUI, not a browser |
| Open WebUI | Owner workbench only, English, frozen for the v0 sprint. Never the doctor's screen |
| Open WebUI Computer (`cptr`) | **Never** on a practice machine. Possible evaluation later on a disposable machine |
| Rebadging Open WebUI as our product | Forbidden — its licence does not allow it beyond small use |
| Replacing the practice software | No. Medilink stays alongside, as today |
| Writing into Medilink or sending via MSSanté | **No.** "Export" produces an artefact; she transmits it herself |
| Ameli professional account (sick leave, occupational disease, work accident) | **No** |
| Accounting module | **No.** E-invoicing and the accountant's software stay theirs |
| Certificates (sport, school, sick child, free-form) | **Out** of the first prototype — Medilink templates already suffice |
| Referral letters | Already in Medilink with automatic history. **Not** cloned |
| Biology follow-up banner | **No** — that lives in Medilink and we do not write there |
| Duplicates (MSSanté and paper) | **Yes** — flagged in the plan, local fingerprints, human decision |
| Permanent delete | **No** in v1. Dedicated trash folder only |
| Allow-list equal to all of Documents | **No** — a dedicated work folder |
| Work folder under Documents | **No**, corrected 18 September 2026. Windows Known Folder Move and iCloud "Desktop & Documents" mirror Documents to a cloud service. The folder is `~/AssistantCabinetAI`, directly in the home |
| A folder any cloud client synchronises | **Refused in code**, with the product named on screen, re-checked at every launch. No opt-out. `docs/PRIVACY-AND-SECURITY.md` |
| Local retrieval index location | `%LOCALAPPDATA%` (`app_local_data_dir()`), never roaming `%APPDATA%`: the index holds document text, and a roaming profile copies `%APPDATA%` to a server |
| A large model on the 2019 practice PC | **No.** A thin client, yes — otherwise no local file writes are possible |
| Copying patient files onto the Mac mini | **No.** It would become a health-record store |
| Real patient documents | **Not** before a written DPIA draft, a named data controller, and disk encryption |
| Consumer assistants (ChatGPT and similar) for practice files | Closed: forbidden, and the pilot understands why |
| Reporting what a document states | **Yes**, including findings, treatments, identifiers and what a letter calls a pathology, **with a citation**. That is reading assistance for the GP. There is no extra clinical filter on top of retrieval |
| The model's own diagnosis, prescription or clinical advice | **No**. The practitioner remains the sole decision maker. The disclaimer on screen is that rule, not a reason to hide what a document already says |
| Interviewing other professions (lawyer, notary) | **Yes**, as information only, no selling, no client data. Method and register: `docs/DISCOVERY.md`. Blank forms versioned, completed answers private |

## Technology

| Subject | Decision |
| --- | --- |
| Client | Tauri 2 with React and TypeScript. Rust only for native work. Not Electron (fallback only) |
| Server | Python, FastAPI, Pydantic — `apps/server`, the single gateway |
| Inference | Ollama behind the gateway, provider-agnostic through `AIProvider` |
| Server location for v0 | A Mac mini **at the practice**, on her LAN |
| Environment | Docker Compose, the same file on Windows and on the Mac mini |
| Retrieval | On the workstation: SQLite full-text plus vectors, behind a replaceable interface |
| Embeddings | No-store call to the gateway first; local ONNX possible later behind the same interface |
| Scan OCR | **In v0 as sprint 2.5**, reversed 19 September 2026 (was "out of v0"). Local only, behind the `OcrProvider` port. When OCR cannot read a page the old rule still applies: report it and refuse to classify, never guess |
| Model choice | Alias allow-list only, licence record mandatory, no clinical-care weights |
| Language | One `locale` variable owned by the client. English prompts in, user's language out |
| Documentation language | English, as of 16 September 2026 |

## Settled while building the vertical slice (sprint 1, 17 September 2026)

| Subject | Decision |
| --- | --- |
| The webview's only bridge | Tauri commands. Sprint 1 has five: snapshot, save settings, choose work folder, check health, send message. The server URL, the alias, the locale and the allow-list stay in Rust |
| Streaming to the interface | A Tauri `Channel` emitting `delta` then `completed`, not polling and not a webview fetch |
| Errors across every boundary | Rust and Python return a machine code plus structured data; the React catalogues turn it into a sentence. A code without a catalogue entry fails a guard test |
| Settings location | `settings.json` in the Tauri app-config directory, resolved at runtime. Never a written path |
| Work folder allow-list | Built from what the platform reports (home, Documents, Desktop, Downloads), not from hardcoded strings, so one rule set covers Windows and macOS |
| `MODEL_ALIASES` format | `alias=model` pairs **or** JSON, because Compose cannot interpolate a default containing braces. An empty map serves nothing rather than guessing an installed model |
| Register storage | In memory with a capacity bound. Metadata and SHA-256 hashes only, never prompt or answer text, not even in debug |
| Guard tests as policy | A French literal or a non-ASCII character in `apps/server` or `src-tauri`, a user-visible literal in a component, and a catalogue key mismatch each fail a test rather than a review |
| Open WebUI's route to the model | Through the gateway (`OPENAI_API_BASE_URL`), with `ENABLE_OLLAMA_API=false`, so the workbench sees aliases and not the model shelf |

## Settled by the sprint 2 architectural reframe (18 September 2026)

Full reasoning, and the inspection of `LocalGridMind` that produced it: `docs/SPRINT-2-ASSESSMENT.md`.

| Subject | Decision |
| --- | --- |
| Tabular data (`.csv`, `.xlsx`) | A **first-class source**, generic and profession-independent, alongside PDF / DOCX / TXT / MD. Not a finance feature, not a GP feature |
| Where the deterministic tabular engine runs | **Rust, on the workstation.** The server must never hold a workbook, so the engine that reads one cannot live there |
| `LocalGridMind` | A **specification and test corpus**, not a dependency. Its engine is Python and pandas; a Python sidecar inside a Windows installer four weeks before deployment on a 2019 PC is rejected |
| Sprint 2 | **Splits.** 2a is documents and carries milestone A on 30 September. 2b is tabular, dated after milestone B, because the pilot's measured workflow contains no spreadsheets (`docs/PILOT-GP.md`) |
| XLSX scope for the first implementation | **Data-grid sheets only.** A formula-heavy workbook is inventoried, disclosed and refused for factual answers. No formula evaluation, no Excel engine |
| A formula's cached result | **Never** presented as a verified fact. `derivation` distinguishes `Extracted`, `Computed`, `FormulaStored` and `ModelAsserted` |
| Question classification | A **locale pattern pack loaded as data**, never regexes or sentences in Rust — which the language guard test would fail anyway. Operations are language-neutral; the React catalogues write the sentence |
| Deterministic answers and the model | A deterministic answer makes **zero** gateway calls, and every deterministic question still answers with the gateway stopped. Proved by tests, including a provider double that panics when called |
| Copying source files into an app store | **No.** Inventory by path plus SHA-256. The work folder is the corpus; we do not duplicate it |
| Long-term direction | Lives in `docs/PLATFORM-VISION.md`, without dates. `docs/ROADMAP.md` stays operational and concrete |
| Embedding weights | Their own alias (`cabinet-embed`) out of the **same** allow-list, with `DEFAULT_EMBEDDING_ALIAS` naming it. Changing it forces a re-index: vectors from two models cannot be compared |
| A batch that comes back the wrong shape | **Refused**, not stored. A short or ragged batch would bind each chunk to the wrong vector, and the index would cite the wrong passage for as long as it lives |
| What `/v1/embeddings` records | Alias, counts, characters, dimensions and one SHA-256 over the batch, in an entry type of its own. Widening the chat entry instead would have loosened a closed field list |

## Settled by the sprint 2.5 OCR reframe (19 September 2026)

Recorded direction: `docs/BRIEF-SPRINT-2.5-OCR.md`. Full reasoning and the repository inspection that
produced it: `docs/SPRINT-2.5-ASSESSMENT.md`.

| Subject | Decision |
| --- | --- |
| Scan OCR in v0 | **Yes**, as sprint 2.5, 20–30 September, in the window sprint 2a vacated by finishing early. Sprint 3 cannot summarise, classify or name a document the pipeline cannot read, and she receives 10 to 20 scans a week |
| Where OCR lives | **Beside extraction, on the workstation**, behind `OcrProvider`. The same rule as the tabular engine: the server must never receive a document, so the code that reads one cannot live there |
| OCR is a workflow feature | **No.** It is an ingestion capability. A GP workflow consumes the normalised document and contains no OCR code, so it never has to change for a new input format |
| Cloud OCR of any kind | **Forbidden**, permanently, on the same grounds as cloud inference and cloud speech. No external document-processing service, no mandatory Internet access |
| Recognised text on disk | **Only in the local index.** The OCR engine must be driven through stdin and stdout, never its default "write the result next to the input" mode. A test asserts no file appears outside the index |
| `Extracted` versus OCR | A **fifth `derivation` variant, `Recognised { engine, confidence }`**. What a machine read is not what a file states, and the difference is a field rather than a choice of words |
| Low-confidence OCR | **Does not enter the index**, so it cannot be retrieved and cannot become a confident model sentence. Uncertainty is resolved at ingestion, where it is still measurable. Confidence is kept internally; no score on screen |
| Text-layer detection | **Per page, not per document.** A born-digital page is never rasterised. A mixed document costs one OCR call per scanned page. The character floor is empirical and measured against the fixtures |
| OCR caching | **No new cache.** The existing SHA-256 skip already prevents re-reading an unchanged scan. Two columns, `ocr_engine` and `ocr_engine_version`, invalidate only what an engine change affects |
| A missing OCR engine | **Degrades to sprint 2a behaviour** and reports `ocr_unavailable`. It never fails the launch and never guesses |
| Deployment as a selection criterion | **Yes.** An engine that makes the Windows installer unmaintainable is rejected however good it is, and the bundle is verified on Windows during sprint 2.5 rather than in sprint 4 |
| DOCX to PDF, and document conversion generally | **Out.** A separate future capability. It must not expand sprint 2.5 |
| The first OCR engine | **Tesseract 5 as a bundled sidecar**, with `pdfium-render` for rasterisation and `fra.traineddata` shipped as a resource. Confirmed 19 September 2026 |
| **Windows and macOS are both mandatory** | Confirmed 19 September 2026, and it now outranks convenience in every technology choice. An engine, library or runtime that exists on only one of them is disqualified however good it is. This is what rejected `Windows.Media.Ocr`: it is two engines with two accuracies and two failure modes, not one |
| Platform differences in OCR code | **None.** No `#[cfg(windows)]` anywhere in `src/ocr/`. A platform difference belongs in the bundler configuration or in sidecar discovery, never in the recognition path, or the two clients drift apart and only one is ever tested |
| Cross-platform CI | **Required, and before the OCR code.** Nothing in this repository has ever been compiled or tested on macOS: there is no CI at all. Until a job builds and tests on `windows-latest` and `macos-latest`, portability is an opinion. `docs/SPRINT-2.5-ASSESSMENT.md` section O |
| Hand-supplied scan fixtures | **Welcome, and better than generated ones**, because a real scanner produces the skew and noise that decide whether recognition is usable. **Fictional content only** — `fixtures/` is committed and permanent, the legal gate is not passed, and a redacted real report is still a real report. Print an invented letter and scan the paper. A genuine practice scan, if ever needed to diagnose a problem, stays in the gitignored `fixtures/gp-sandbox/real/` tree |
| Tabular data (`.csv`, `.xlsx`) | **Confirmed as sprint 2b, after sprint 4, and not optional.** The pilot GP has no spreadsheets, so it earns nothing for milestone B and stays out of the pilot window. It is not speculative either: hospital staff who schedule from these files are a named user group. A later sprint may not quietly drop it, nor reduce it to "spreadsheets flattened into text chunks" — it ships to the standard in `docs/SPRINT-2-ASSESSMENT.md` |

## Settled while building the OCR engine (20 September 2026)

Found while wiring `TesseractProvider` and `Rasterizer` (`src/ocr/tesseract.rs`, `src/raster.rs`). None
of these reverse a decision above; they are the operational detail that decision needed.

| Subject | Decision |
| --- | --- |
| Committing the Tesseract sidecar, its DLLs, `fra.traineddata` and `pdfium`'s library | **No.** `fra.traineddata` is a model weight in the same sense as an Ollama `.gguf`, and the engine binaries are the same class of large third-party blob. `AGENTS.md`'s git rule already forbids model weights; `.gitignore` now excludes `apps/desktop/src-tauri/binaries/*.exe` and the three `resources/{tesseract,tessdata,pdfium}/` trees the same way it excludes `models/*.gguf` |
| How a developer or CI gets those files | `scripts/fetch-ocr-resources.ps1`, run once, reproducibly: downloads the UB-Mannheim Tesseract installer, installs it, downloads `pdfium-binaries` and `tessdata`'s `fra.traineddata`, and stages all three into the gitignored trees above. A macOS sibling script does not exist yet |
| `bundle.externalBin` / `bundle.resources` in `tauri.conf.json` | **Not wired yet, deliberately.** `tauri-build`'s build script validates every declared path on *every* `cargo build`/`cargo test`, not only `cargo tauri build`. Declaring it before the fetch script has run breaks the crate everywhere except the machine that just ran it — including `.github/workflows/ci.yml`'s `windows-latest` and `macos-latest` jobs, which do not run it. The exact config, verified once by a real `cargo tauri build --debug` that produced a working MSI and NSIS installer, is recorded in `apps/desktop/src-tauri/binaries/README.md` so it does not have to be rediscovered. It goes back into `tauri.conf.json` together with a CI step that fetches the resources first, on both platforms |
| Which Tesseract DLLs the sidecar actually needs | Verified empirically by trial removal, not assumed: 30 of the roughly 60 the installer ships. The half that is *not* needed — `libpango*`, `libcairo*`, `libglib*`, `libgio*`, `libgobject*`, `libgmodule*`, `libicu*75.dll`, `libfontconfig*`, `libfreetype*`, `libfribidi*`, `libgraphite2*`, `libharfbuzz*`, `libthai*`, `libdatrie*`, `libpixman*` — belongs to the installer's bundled `text2image` tool, not to recognition |
| Sidecar DLL placement | A bundled sidecar's dependency DLLs must sit in the **same directory as the sidecar executable**, not under `resources/`. Windows resolves a spawned process's DLL dependencies by searching its own directory (plus `PATH` and the system directories) before anything Tauri's resource system provides. `pdfium.dll` has no such constraint: our own code loads it from an explicit resolved path, so it can live anywhere under `resources/` |
| Real OCR proven end to end, once | `tesseract.exe`, staged by the fetch script with its verified minimal DLL set, correctly read `fixtures/gp-sandbox/inbox/2026-03-26_ordonnance-scan.png`'s fictional French prescription text on this Windows machine. Encouraging, but one run on one machine, not the sprint's test suite (section M of `docs/SPRINT-2.5-ASSESSMENT.md`) |
| macOS OCR resources | **Not started.** No fetch script, no verified dylib set, no build or test ever run on macOS for this repository at all (`docs/SPRINT-2.5-ASSESSMENT.md` section O still applies unchanged). Homebrew's `tesseract` formula and `pdfium-binaries`' `pdfium-mac-{x64,arm64}.tgz` are the two known sources; the dylib-set verification needs the same trial-removal method this session used on Windows, done with `otool -L` instead of deleting files and re-running |

## Settled by sprint 2a.5, work folder intelligence (23 September 2026)

Design and non-goals: `docs/WORK-FOLDER-INVENTORY.md`.

| Subject | Decision |
| --- | --- |
| Who owns filesystem facts | The **work folder inventory**, built on the workstation from `std::fs` plus the local index. A file count, a file name, an extension, a path, a readable/unreadable state or an indexed state is **never** inferred from a retrieved chunk and never written by a model |
| File identity | The existing content SHA-256, the value `Source.origin.sha256` already carries. No second hashing model. A renamed or moved file keeps its identity; two byte-identical copies share it, and the inventory reports both rather than choosing |
| `FileRecord` versus `Source` | `FileRecord` answers "which file is this", `Source` answers "which evidence from it supports this answer". Neither replaces the other, and `Source` is unchanged |
| What may live on `FileRecord` | Identity, filesystem metadata, a generic classification, processing state, and an optional domain summary. **Not** page counts, chunk counts, OCR confidence, sheet dimensions or column types: those belong to the domain layer that produced them, which is what lets sprint 2b reuse the record |
| Spreadsheets in the inventory | Recognised as `FileKind::TabularCandidate` and **not** ingested. No CSV or XLSX parsing, and no fake document chunks for a spreadsheet, until sprint 2b |
| An ambiguous file reference | A **result**, never a tie to break. Two files with the same name produce a question and no retrieval. Ordering is never used to pick one |
| A reference outside the work folder | Refused on the string, before any lookup, so no path outside the folder is built, opened or stat-ed. A file name is data, never an instruction |
| Filesystem questions and the gateway | Counting, listing, listing by extension and showing the folder structure make **zero** gateway calls and still answer with the server stopped. Proved by a provider double that counts every request |
| Question vocabulary | A **locale pattern pack loaded as data**, one per catalogue, as sprint 2b's will be. A question takes the deterministic path only when every one of its words is in the pack, so an ordinary content question falls through to retrieval |
| Wording of a deterministic answer | Rust returns a machine code plus facts; the React catalogues write the sentence. An answer no model wrote carries no model label |
| An explicitly named file | Constrains retrieval to that file. A question naming no file keeps the existing whole-folder behaviour |
| A question about **every** document | Routed to a per-document pass: the inventory supplies the list of indexed files and retrieval runs once inside each, so no indexed document is left out by a similarity ranking. Whole stored chunks only, deterministic order, and a file that ranks badly contributes its opening passage rather than being dropped |
| Saying how much of the folder an answer covers | Computed in Rust from the evidence and the inventory, rendered by the interface **beside** the answer. A model cannot omit it. Shown only for a question that asked about every document, since a single-fact question is properly answered from one passage |
| What the model is told about the folder | A generated, compact `WORK_FOLDER_CONTEXT` block plus a knowledge contract, appended to the existing system prompt rather than replacing it. No absolute path, no content hash, no file size and no document text; the view is the smallest one the question needs |

## Settled while testing models on the workstation (23 September 2026)

| Subject | Decision |
| --- | --- |
| What bounds an answer in the client | **Silence, not duration.** The 300 s total deadline in `gateway.rs` is replaced by an inactivity bound: an answer that keeps arriving is never cut off, however long it takes, and one where nothing arrives for the configured wait is abandoned. A total deadline killed a healthy `ministral-3:3b` answer mid-flow on the pilot workstation |
| Why that is safe now and was not in September | The runaway-loop case the total deadline used to catch is bounded elsewhere: the gateway caps every answer at `MAX_OUTPUT_TOKENS` (2 048), and the interface has a stop under the answer being written (`docs/TROUBLESHOOTING.md`, 22 September). The client no longer needs to be the last resort |
| Where the wait is set | A **setting**, `answerIdleTimeoutSeconds`, default 300, clamped to 30–3 600 in Rust on the way in and on the way out. How long a model stays quiet depends on the model and on the machine, neither of which is knowable from the code |
| A partial answer when the AI goes quiet | **Kept**, marked incomplete, exactly as a stop keeps it. Deleting it was an asymmetry: the same half-written text survived her stop and was thrown away by a timeout. The two are told apart on screen - one was her decision, the other was not |
| Dismissing an error | A banner reporting a moment that has passed can be closed. One reporting a state the software is still in, such as a work folder that no longer passes the rules, cannot: closing it would hide something still true |

## Settled while testing models on the workstation, part two (23 September 2026)

Measured causes and the reasoning: `docs/WORK-FOLDER-INVENTORY.md`.

| Subject | Decision |
| --- | --- |
| When a question is answered from the folder | An **intent** and a **subject**, no **content word**, and no unknown word that the documents themselves contain. The earlier rule - every word must be in the pack - failed on four ordinary French words and sent a listing request to a model that then mistranscribed three of sixteen paths |
| How a content question is told from a folder question | The **corpus**, through a `CorpusWords` port over the existing FTS index. `metformine` is a word in the documents, `disponibles` is not. No dictionary is maintained, and the routing stays testable without a database |
| Words that mean "read this to me" | A short **denylist** in the pack (`summary`, `mention`, `resume`, `contient`, ...), disqualifying the deterministic path outright. A denylist because the ways of asking for content are few and the ways of asking politely are not |
| File versus document, in the interface | A **file** is anything on disk; a **document** is a file something could be read from. "Tous mes documents" means the ones that were analysed. Both words live in the pattern pack and the panel uses them the same way |
| A shortened file name | **Resolves.** People drop the date, not the subject: `12_compte-rendu-biologie.pdf` reaches `inbox/2026-03-12_compte-rendu-biologie.pdf`. A file actually called the reference wins over one that ends with it; fragments match names, never paths; an extension alone is not a reference; several tails matching is an ambiguity like any other |
| Saying an answer was computed | A deterministic answer carries its own provenance line where a model answer says "generated by". Each listed file carries what happened to it, in the same words the panel uses |
| Reaching the model anyway | The **existing** regenerate control, relabelled on deterministic answers: recomputing them would return the same bytes, so there the button asks the model instead. No new control - the window has no room for one |
| What the model is sent for a per-document question | Counts, not the file listing. The excerpts already carry one header per file, and a second copy of every path pushed the first excerpts into the middle of a long prompt, which is where small models were observed losing them |

## Known blind spots to keep in mind

- **File actions are the number one business risk.** A bad batch rename over hundreds of documents is far
  worse than a bad chat sentence. Dry run, manifest, cap, hashes, trash, no permanent delete.
- **Locks.** Word, the antivirus and OneDrive lock or resynchronise files. Handle the failures explicitly
  rather than reporting success.
- **The practice's real naming convention wins.** The AI's "ideal" tree can break the existing filing.
  Collect the pilot's actual convention before writing a `move` tool.
- **Extraction quality decides everything.** Many practice documents are scans. Without good OCR the answers
  lie, so measure the empty-page rate before promising intelligent filing.
- **Concurrency.** A 64 GB Mac mini holds few large contexts at once. Without a queue, timeouts and a fast
  default model, the second user concludes the AI is broken.
- **"Open" weights are not automatically usable in a practice.** Check commercial use, health or legal
  restrictions, and attribution for each one, next to its Ollama name.
- **Shared accounts destroy accountability.** Per-person keys from the first multi-workstation setup.
- **Do not couple the business logic to an agent harness.** The differentiator is the workflow, the plans and
  the audit, not the latest runtime.
- **A failed trial worth remembering.** A French-only filing prompt made the model invent tool calls
  (`rename_file`, `move_file`, `delete_calendar_event`). The fix was English instruction bodies, an explicit
  ban on tool calls, a Markdown-table-only output, and `function_calling: legacy` in `compose.yaml`. This is
  why prompts are English and why file actions live in code rather than in model output.

## Reserved contracts, deliberately not implemented

Voice and mobile are anticipated as contracts only. No implementation before the document pilot holds.

```text
POST /v1/audio/transcriptions   → text  (no-store: no audio kept)
POST /v1/audio/speech           → audio (optional)
POST /v1/jobs                   → { intent, text, bind: "workstation", dry_run }
GET  /v1/jobs/{id}              → status, never document content
```

Rules already frozen for them: cloud speech-to-text and text-to-speech are **forbidden** (local or on-device
only); audio is never retained; a voice command must never trigger `apply` on files, only a draft or a plan
needing visual approval; audio counts as one inference slot; a phone is a remote control and a dictation
device, never a second document store; the mobile surface is a LAN progressive web app, never an app store
release in v1. Handlers return `501` until a dedicated phase.

## Client UI — noted, not scheduled

**Expanded 21 September 2026 into `docs/CHAT-UX-ASSESSMENT.md`**, which inspects the code behind each of
these and adds markdown rendering, a stop button, file upload and dictation. The three rows below keep
their decisions; the assessment carries the evidence, the effort and the open questions.

| Subject | Decision |
| --- | --- |
| Primary accent colour (green → blue) | Not changed. `--accent` in `apps/desktop/src/styles.css` and `BACKGROUND` in `apps/desktop/src-tauri/icons/generate-icons.py` are the same RGB (`#1f5d54` / `(31, 93, 84)`), so a colour change is a two-file edit plus regenerating the PNG/ICO set — easy, but it touches the app icon too, so it waits for an explicit go-ahead rather than sneaking in with an unrelated change |
| Per-answer action row (copy to clipboard, resubmit, read the answer aloud) | Not on the roadmap yet, and not implemented now. These are standard on public chat UIs and are worth adding once OCR (Sprint 2.5) is done, before the sprint-3 workflows. Copy sits closest to the existing "export" concept (`docs/ROADMAP.md`); read-aloud would need `docs/DECISIONS.md`'s voice rules (local TTS only, `docs/DECISIONS.md`'s "Reserved contracts" table) revisited for text already on screen rather than a live conversation |
| Edit-and-repost a past question (ChatGPT/Gemini style) | Feasible, small: `entries` already holds every user turn in `apps/desktop/src/state/useChat.ts`, so a per-turn "edit" affordance in `MessageList` would populate the composer and truncate `entries` from that turn on resend — no new IPC, no server change. Not implemented now, tracked alongside the action row above |
| Incremental indexing (add one file to the Work Folder without re-analysing the whole folder) | Feasible, larger: `IndexStore` already keys chunks by file path plus a content hash and skips unchanged files on a full `indexWorkFolder()` pass (`workFolder.unchangedOne/Many` in the summary), so the incremental *engine* exists. What is missing is (1) a Tauri command that indexes one dropped/copied file instead of walking the whole folder, and (2) a drop target or file picker in `WorkFolderCard` that copies the file into the Work Folder first — the allow-list only covers what is already inside it (`docs/PRIVACY-AND-SECURITY.md`). Worth scheduling once the OCR chain (Sprint 2.5) and the retrieval tests hold, not before |
| Markdown rendering in an answer | **Done, 21 September 2026.** `react-markdown` plus `remark-gfm` (tables, because a summary of a specialist report often comes back as one), building a React tree rather than injecting HTML, since model output carries extracted document text. A link and an image are stripped to their own text, which is how the row below is enforced rather than merely intended. `docs/CHAT-UX-ASSESSMENT.md` item 1 |
| A download or source link **inside** the model's markdown | **Refused.** A path chosen by model prose is the failure recorded above under "A failed trial worth remembering": file actions live in code, not in model output. Anything naming a file travels on the structured channel beside the text — as retrieval sources already do — and Rust re-validates it against the work-folder allow-list. `docs/CHAT-UX-ASSESSMENT.md` item 1b |
| Stop button during generation | **Done, 21 September 2026, deliberately narrow.** The stop below the composer is offered **only while the answer has not started** — one button in one place, reading `Stop` (the same word in French, shorter and universally understood) while the spinner shows and `Envoyer` again, disabled, the moment the first words arrive. So a half-read answer is never taken away from her, and that stop keeps nothing on either side: the question goes with the answer and her text returns to the composer. It is not a view-only change — Tauri v2's `invoke` has no abort — so `cancellation.rs` holds the signal and drops the run's future, which closes the connection and is what actually stops the model; it wraps the whole of `ask_with_sources`, so the embedding leg cancels too. The gateway needed no change. `docs/CHAT-UX-ASSESSMENT.md` item 2 |
| A second stop, **under the answer being written** | **Done, 22 September 2026**, after a 0.5B model wrote the same invented block for three and a half minutes with nothing on screen to stop it (`docs/TROUBLESHOOTING.md`). Two stops, because they are two acts: before the first word nothing is kept, after it everything is kept, marked `chat.interrupted` rather than left looking finished. `stopOutcome` in `src/lib/generation.ts` decides which, recorded at the click since the phase has moved on by the time the run ends. The retrieval command now sends its `Sources` event before the answer rather than after, so an interrupted answer still cites its documents. **This is the escape hatch, not the fix** — see the row below |
| A ceiling on how long an answer may be | **Done, 22 September 2026.** `MAX_OUTPUT_TOKENS`, default 2048 (~6 500 French characters, against ~1 300 tokens for a real summary), applied in `capped_output_tokens` whether or not the caller sent `max_tokens`: a caller may ask for less, never for more, and `-1` — "unbounded" to llama.cpp and Ollama — is read as asking for nothing. The counterpart of `MAX_CONTEXT_CHARS`, and the same principle: **the gateway does not trust the caller**. It is a safety property, not a tuning knob: `LLM_REQUEST_TIMEOUT_SECONDS` is the longest gap *between* chunks, so no timeout can end a model that loops steadily, and before this existed one wrote the same invented block for three and a half minutes. Unlike an oversized context it is not an error — a request for a longer answer is still reasonable, it simply gets a shorter one. Sub-1B weights also do not belong in `MODEL_ALIASES`: an alias in the settings is a promise that it works. `docs/TROUBLESHOOTING.md`, 22 September 2026 |
| Dictating a question (speech to text) | **Not before 14 October 2026**, and not via the browser. Chromium's `webkitSpeechRecognition` streams audio to Google's servers, which the cloud-speech ban forbids outright, and it is unavailable in WKWebView, which fails the Windows-and-macOS gate. The local path is a `SpeechProvider` port with a bundled `whisper.cpp` sidecar — the same shape and the same cost as Sprint 2.5, so a sprint rather than a feature. `docs/CHAT-UX-ASSESSMENT.md` item 7 |

| What the folder is **called on screen** | **Changed 24 September 2026: "Documents folder" / "Dossier des documents".** A second, separate folder for tabular files is coming, so one screen cannot go on calling this one "the work folder" — the name has to say which of the two it is. The card also stopped claiming to be "the only folder this software may write into", which the tabular folder is about to make false; what it is *for* (PDF, DOCX, even JPEG) plus the dedicated-folder rule moved into the path box's `title`, leaving the heading, the path and the buttons on screen. The change is **catalogue only**: `workFolder.*` keys, the `workFolder` setting, the `work_folder_*` error codes, the Rust modules and `docs/WORK-FOLDER-INVENTORY.md` keep the name they have, so no stored `settings.json` and no machine code moves. Every user-visible sentence moved together — the card, the deterministic folder answers, the error messages — because two names for one folder is worse than either name. **Known consequence:** `resources/work-folder-questions/fr-FR.json` lists `travail` as filler and `documents` as a *subject*, so "mon dossier de documents" now resolves to `Subject::Documents` where "mon dossier de travail" resolved to `Subject::Folders`. Both answers are defensible; the vocabulary pack has not been retuned for the new phrase |
| Emphasis on the **Analyse** button | Filled with `--accent` only while `analysisPending` is true — a file is `discovered` (never read) or `pending` (changed since it was read). Once every file is read it is an ordinary bordered button beside "change the folder". A `failed` file does not keep it lit: analysing again would fail again, and a button that is permanently lit teaches her to ignore it. `apps/desktop/src/lib/analysis.ts` |
| Progress during an analysis pass | **Done, 24 September 2026.** `index_work_folder` takes a `Channel<IndexProgress>` and `indexing::run` reports `{processedFiles, totalFiles}` before the first file and after the last, so a folder of scans shows a hairline filling rather than an unbroken spinner. **Counts only — no file name and no document text crosses that channel**, so a progress report can never become a second place content leaks to. The bar is removed when the pass ends rather than parked at 100% |
| The **Analyse** button under an unanswered question | When retrieval refuses because nothing has been analysed, that turn offers *analyse* instead of *copy* and *regenerate*: both of those would reproduce the same sentence, and the one thing that answers her question is the pass she has not run. A successful pass then re-asks her question automatically; a failed one stops, with the error already on screen in the folder card. The pass is owned in `useIndexing` above both places that start it, so the card's button and the answer's button are one pass, not two |

| Noticing a file added to the folder | **Window focus, not a filesystem watcher. Decided 24 September 2026.** The panel rebuilds when the window regains focus, which is the moment she comes back from Explorer or Finder having dropped a document in — the actual gesture. A watcher (`notify`) works on both platforms but brings a background thread, debouncing a scanner writing one file in several pieces, and a cloud-sync folder churning underneath it, and buys only the same answer a few seconds sooner. Rejected for v0 on the reliability rule in `AGENTS.md`, not on feasibility. Focus refresh is affordable because `inventory::FileHashCache` re-reads a file's bytes only when its size **and** modification time have moved |
| Analysing automatically when a file appears | **No.** A pass sends chunk text to the gateway for embeddings and takes minutes on a folder of scans. Anything that sends document text stays an explicit, visible act she starts — the same principle as "file actions = plan + human approve" |
| **Reset** — emptying the local index from the interface | **Added 24 September 2026**, beside the folder controls, behind a confirmation. It exists because the only route back to "nothing analysed" was closing the application and deleting `%LOCALAPPDATA%\com.assistantcabinetai.desktop` by hand (`docs/TROUBLESHOOTING.md`). It empties the index and the hash cache; it never reads, moves or deletes a document. Not undoable, and deliberately not offered as such: re-analysing is the undo |
| The confirmation in front of a destructive action | **Built, not the operating system's.** A native confirmation carries the system's words in the system's language, which is not necessarily hers, and breaks the one rule the product does not bend (`docs/LANGUAGE-AND-LOCALE.md`); it would also look like Windows on Windows and macOS on macOS inside a window that looks like neither. `ConfirmDialog` is small, centred and generic — the sprint-3 file actions need exactly this shape |
| **See** / **Voir** — opening the folder in the file manager | **Added 24 September 2026.** The command takes **no path**: Rust reads the folder from settings, so the webview asks for "the work folder" and never for a folder of its own choosing, and the allow-list holds because there is nothing to allow-list. `reveal.rs` is the only file in the crate that names a file manager, with paired `#[cfg]` arms for Windows (`explorer.exe`) and macOS (`open`) side by side and a third returning a machine code rather than guessing |
| Folder-card button labels | **One word each, 24 September 2026.** Four controls now share one row in a sidebar column — *Voir*, *Changer*, *Reset*, *Analyser* — so "Changer de dossier" became "Changer", with the full sentence on hover (`workFolder.changeHint`). The card's own heading already says which folder it is. The row distributes leftover space equally from each label's own width rather than using `space-between`, which left four buttons of four different widths adrift; `flex-wrap` stays as the last resort for a longer translation. The **Voir** control was first placed beside the path box and moved after seeing it: a full-height button against a two-line monospace path made the card look broken |
| **Réinitialiser** — settings back to a first launch | **Added 24 September 2026, and it resets the documents folder too.** A narrower version keeping the folder was proposed and refused by the owner: a reset that only covers the settings that are easy to retype is not a reset, and the folder is the setting most likely to be part of whatever went wrong — a path on a disk that is no longer there, or a folder a sync client has since taken over. Same `ConfirmDialog` as the index reset. The defaults come from Rust (`reset_settings` returns `Settings::default()` through the ordinary save path, so validation still applies), because `DEFAULT_SERVER_URL` can come from the environment and a default must be read, never assumed by the interface. **The local index is not touched** — the confirmation says so, and choosing the same folder again finds every document still analysed |
| The folder card in two places, one markup | **Settled 24 September 2026**, after the settings panel showed the card at full dialog width beside fields capped at `44em`. The card now takes the same ceiling (`.dialog__body > .card`), written against the panel rather than as a modifier the card wears, so anything dropped in there later lines up without being asked to. The file listing has **two shapes for one markup**, chosen by which of its two homes it is in (`detail`) rather than by a breakpoint — the card's width is decided by where it is, not by the size of the window. Wide: the state beside the name, states aligned down the right, which is how a listing is normally read. Sidebar: the state underneath, because at 300px the same two columns leave the name a few characters wide. In the button row **Analyser takes three shares of the spare width to the others' one** — it is the control that matters, so it is the one that gets the room, and the ratio reads as "largest" at both widths rather than only the wide one |
| Changing the work folder and what happens to the index | Choosing a different folder does **not** empty the index — and before 24 September 2026 that meant the previous folder's passages stayed citable for ever. They are now dropped by the first pass over the new folder. **Consequence:** after switching folders the index still describes the old one until she presses Analyse. Deleting `settings.json` by hand therefore loses no analysis: re-picking the same folder finds the index exactly as it was |
| Dropping index rows for a file removed from the folder | **Done, 24 September 2026**, at the start of every pass. Until then a deleted document's rows outlived it: the panel stopped listing it, because that reads the filesystem, while retrieval went on offering its passages as evidence for an answer. The pass names what it forgot, because only she can tell a deliberate deletion from a folder that failed to mount |

## Out of scope until the pilot holds

Fine-tuning, mobile applications, a multi-practice hosted service, autonomous overnight operation, a cloud
fallback model on real files, a server-side case index, irreversible actions without review, a second shared
profession, wide-area multi-site access outside a practice VPN.

Asking a second profession questions is not opening one. Discovery findings wait in `docs/DISCOVERY.md` until
the GP pilot holds.
