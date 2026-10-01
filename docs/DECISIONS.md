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
| Documents live at the work folder's root | **No**, decided 28 September 2026, ahead of CSV/XLSX work. The suggested and allow-listed folder is `~/AssistantCabinetAI/Docs`, a dedicated subfolder, so a sibling `~/AssistantCabinetAI/Data` for spreadsheet work can be added later without the two kinds of file sharing one directory. The sidebar shows the abbreviated path (`…\Docs` / `…/Docs`); the settings panel still shows it in full. *(Subfolder casing corrected the same day from `DOCS`/`DATA` to `Docs`/`Data`, see the Data Folder UI session below.)* |
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

## Settled by the analysis scope layer (25 September 2026)

Prerequisite for Sprint 2b's tabular engine, not part of it. Code: `analysis_scope.rs`.

| Subject | Decision |
| --- | --- |
| What a scope is | *(Default and empty-scope rule superseded on 27 September 2026, see "document selection and conversation memory" below.)* An **optional narrowing of the work folder allow-list**, defaulting to the whole folder (`ScopeMode::WholeFolder`, today's behaviour expressed as a case of the type rather than a path around it). It only ever picks among files the inventory already holds and copies nothing. It is **not** a per-conversation upload, which `docs/SPRINT-2-ASSESSMENT.md` section E rejects for a different reason |
| How it is enforced | `AnalysisScope::resolve(&inventory)` returns an **inventory** holding only the members. The router, the file-reference resolver, the counts and the Work Folder context are all built on an inventory, so none of them can look past the scope without scope-specific code. Retrieval is held to the same members through `RetrievalScope::Files`. An explicit scope with no survivors allows **nothing**, never everything |
| A pinned file that changed | Each entry pins `FileRecord::id` when it is chosen. An entry whose file vanished (`missing`) or no longer has that content (`changed`) is **left out, not answered from**, and reported to the interface as `scopeOutdated`, which writes a line under the answer. Answering from a file other than the one she chose would be the quiet failure the scope exists to prevent |
| What is deliberately not stored | No `scope_id` (nothing persists a session yet), no `allowed_domains` (derived from members' `FileRecord::kind` on demand, so it cannot drift from real membership), no purpose label, no status enum, no sheet restrictions (`sheet_names` is added to `ScopeEntry` by Sprint 2b, when something exists to read it) |
| Where it lives | In the client, beside the single in-memory conversation, and it is **lost on restart**. Full `ChatSession` persistence and a conversation list are a separate, larger feature. It is passed to `ask_with_sources` as an optional argument, so an absent scope means the whole folder |
| Deferred | Intent-based proposal ("the file for Mrs X"), which needs semantic matching that does not exist yet. Resetting the scope when the work folder changes |
| Sprint 2b contract | `TabularAnalysis.analyze(query, scope)` takes its scope from `AnalysisScope::resolve`, never from the folder directly, so a tabular question cannot read a workbook the conversation was not scoped to |

## Settled by the clean file names change (25 September 2026)

Code: `filename_sanitizer.rs`, `file_reference.rs`.

| Subject | Decision |
| --- | --- |
| Renaming her documents on Analyse | **Decided by the owner, 25 September 2026, without a confirmation step.** Pressing **Analyser** first renames every document whose name is not clean, then reads the folder. This is a **deliberate exception** to "file actions = plan + human approve", written into `AGENTS.md` rule 3 the same day: a French practice's names carry spaces, accents and two Unicode spellings of the same accent, and a name that can be typed several ways cannot be matched reliably by a question or trusted as one path across a Windows and a Mac. The reasoning was that the files arrive as copies (downloads, e-mail attachments, another program's export), so the original survives elsewhere. **That is an assumption, not a guarantee**: a file dragged out of another folder rather than copied has no other copy, which is why the safeguards below exist |
| What "clean" means | ASCII letters and digits, `-`, `_` and `.`. Accents are removed (`Esaie` for the accented form), `oe`, `ae` and `ss` are spelled out, every other character becomes one `-`, runs collapse, the ends are trimmed. The extension is kept. A stem with nothing left becomes `document`. A clean name is never touched, and cleaning twice changes nothing |
| What it will not do | **Never overwrites**: a taken name (compared without case, as Windows and a default Mac disk do) gets `-2`, `-3`. **Never changes content**. **Never leaves the file's own folder**. **Never renames a folder.** Only files the pipeline reads are considered, so an unrelated file keeps its name; hidden files and Office lock files (`~$...`) are skipped, the second being Word's own bookkeeping |
| Recovering an original name | Every rename is appended to `renamed-files.jsonl` in the application data folder (`at`, `from`, `to`; names only, never document text) **before** the pass starts, so a pass that fails afterwards still leaves the record. The interface lists the first five renames, old name and new, under the analysis summary, and counts the rest |
| A file that cannot be renamed | Open in another program, read-only, or refused by the volume: left exactly as it was, counted in the summary, and still analysed under its old name. Nothing is silent |
| Consequence for the index | A rename changes a document's relative path, so the first pass after it forgets the old path and reads the file again under the new one. A scope that pinned the old path reports it as missing. Both are one-time costs, and the picker only offers files that are already analysed, so a scope never holds a name that is about to change |
| Matching a name she types | Comparisons fold accents and case through Unicode NFD, so the accented, unaccented and two-code-point spellings are one name. A file whose name is several words is also recognised when the question writes them as words (`Absence pour Esaie`), as a whole run of words only, for names of at least eight characters, and the longest name wins when two overlap. Single words are matched exactly as before: nothing looser was introduced |
| Not decided here | Sanitising folder names, and asking before renaming as an option. Both stay possible without changing this design |

## Analysis scope: what was added afterwards (25 September 2026)

| Subject | Decision |
| --- | --- |
| Sources listed once per file | An answer built from three excerpts of two files listed three lines, which read as three documents next to a two-file scope. The list now groups by file and gives the pages (`file.pdf, pages 1, 3`). The evidence sent to the model and the citations it writes are unchanged |
| A scope that holds nothing usable | Refused with `scope_unavailable` ("choose them again"), never with `insufficient_evidence`, because the documents were not searched and found wanting: the ones she named could not be used. Raised without a gateway call when nothing survives, and also when a search of the survivors finds nothing while a chosen file was left out |
| A different work folder | Resets the scope - to **none** since 27 September 2026, to the whole folder before. Paths chosen in one folder resolve to nothing in another |
| The picker | Offers files whose analysis is **current** (`processing_status = indexed`), not merely files that were once indexed, so a scope never pins a file whose stored passages are older than its content |
| A changed pin | `id` falling back to a path hash means the bytes could not be read, and such a file is reported as changed rather than trusted. That is the intended outcome, not a false alarm |

## Settled by the document selection and conversation memory reframe (27 September 2026)

Decided by the owner after a trace of every path a question could take. Reasoning, and what was found wrong:
`docs/SELECTION-AND-MEMORY.md`. Supersedes, where they conflict, the default and the empty-scope rule of the
25 September analysis scope tables above.

| Subject | Decision |
| --- | --- |
| The default selection | **None** ("Documents utilisés : aucun"), at startup and whenever the documents folder changes. The selection is built only from her own actions. Analyse never changes it |
| An empty document selection | **Answered without excerpts**, not refused. Still reads no document: isolation is unchanged, only "no answer" is gone. Decided in terms of *documents* on purpose: when tables arrive, "tables selected, no documents" reaches the tabular engine with no excerpt, and "documents selected, no tables" reaches the document engine with no data or workbook inventory |
| Chose nothing versus chose something unusable | Only a selection with **no document chosen** is answered without documents. A selection whose files are all gone or changed is still refused with `scope_unavailable`: answering without the documents she named would be the quiet failure. A selection whose files lost their analysis (Reset) behaves as before |
| "Tous" | The whole folder, including documents analysed later. Ticking every analysed file one by one **is** "tous". Unticking the last file returns to **none**, never to the whole folder |
| "Tous" searches the current folder only | Retrieval is held to the chunks of files present in the current folder, in the index's own order. Identical results when the index matches the folder; a previous folder's passages, or a deleted file's, can no longer be cited before the next Analyse |
| Deterministic answers with nothing selected | A question about the folder's files answers "no document selected for this conversation" (`nothing_selected`), with no count: the counts are already on screen |
| A named file outside the selection | `file_not_selected`, "this file is not selected for this conversation", instead of the false "no file with that name". For every partial selection, not only the empty one |
| The answer without documents | Its own short English instruction in place of the retrieval one; no embedding call; the line "Réponse sans vos documents." under it |
| Models reciting their rules | The gateway's "say so when the information does not carry the answer" rule is narrowed to questions that depend on the practice's documents, and the base prompt says the rules are not part of the answer. Answers are **never** edited after generation |
| The documents folder | **Mandatory.** The composer is disabled until one is chosen; an empty folder is fine. The separate no-folder chat path (`send_chat_message`) is retired: every question takes `ask_with_sources`. The future data folder follows the same rule; how a first launch offers two folders is for Sprint 2b |
| Conversation memory | Every mode remembers. Past questions and answers are sent as text; excerpts are never resent. Whole exchanges are kept newest first within a budget and dropped oldest first; the current instruction, excerpts and question always come first. Software-written turns ("analyse first") are not remembered |
| The memory budget | The model's context window, minus `MAX_OUTPUT_TOKENS`, at a conservative 3 characters per token, under Rust's 24,000-character ceiling. Computed per question for the model chosen for that question: microseconds, no model call. A small model has a short memory, which is acceptable |
| The context window | Set per alias on the **server device** (`MODEL_CONTEXT_WINDOWS`, default `DEFAULT_CONTEXT_WINDOW` = 8,192 tokens), capped at the model's own maximum, passed on every request, and published per alias in `/health`. Never left to the runtime's default again |
| Measuring | A one-off script on the server device (`apps/server/scripts/measure_context.py`), not a runtime check. Written for any server device, not one machine |
| Follow-up questions | A question of eight words or fewer is searched together with the previous question; the router and the file-name resolver read the current question only. A model rewrite of the question is deferred: it costs a model call |
| Privacy | Unchanged. The history lives in the application's memory and is lost on restart; the gateway stores hashes only |
| Documents and tables together | **Open, for Sprint 2b.** Direction: the engines compute and the model only writes; Rust routes each question; two labelled blocks with separate budgets; one "based on" line per engine (`docs/SELECTION-AND-MEMORY.md`) |

## Settled by the grounding-over-memory correction (27 September 2026, the same day)

The conversation memory shipped hours earlier caused a real hallucination regression: traced to two specific
causes, both fixed the same day. Full reasoning: `docs/SELECTION-AND-MEMORY.md`, "Grounding outranks memory"
and "Profession-neutral model-facing text".

| Subject | Decision |
| --- | --- |
| The honesty rule | **Unconditional again.** `BASE_SYSTEM_PROMPT`'s "you never invent a fact... say so" no longer depends on the model's own judgement of whether a question "depends on the documents" - that judgement call is exactly what let the model reason its way into hallucinating. General-knowledge answers are permitted **only** by `NO_DOCUMENTS_INSTRUCTION`, sent on the one path Rust has already determined has no document selected |
| The grounding priority chain | Three tiers, decided deterministically by Rust, never guessed by the model: (1) a document selected → `RETRIEVAL_INSTRUCTION`, strict; (2) *(reserved, Sprint 2b)* tabular data selected and no document → a future instruction of the same shape, strict; (3) neither → `NO_DOCUMENTS_INSTRUCTION`, meant to be rare and still cautious - general knowledge is a bounded last resort, not a licence to invent, though what the user herself has already said in the conversation is trusted |
| Where the grounding material sits in the request | **Immediately before the question, after every past exchange** - never in a leading system turn separated from the question by history. `commands::Writer::write` folds the tier's instruction and its excerpts/data into the final turn. Small local models weigh nearby turns more than distant ones ("lost in the middle"); the longer a remembered conversation got, the more this let a model drift onto a prior turn's topic or reuse a stale citation instead of the current excerpts |
| Profession-neutral model-facing text | **Standing rule, not a one-time cleanup** (`AGENTS.md`). Every string sent to the model must read the same for a doctor, a lawyer, a notary or an accountant. `BASE_SYSTEM_PROMPT`, `RETRIEVAL_INSTRUCTION` and `NO_DOCUMENTS_INSTRUCTION` no longer mention "practice", "patient", "doctor" or "practitioner"; each carries a guard test that fails the build if one reappears. The one kept word, "clinical" - one of three parallel examples, "clinical, legal, financial or otherwise" - is deliberate: naming it alongside two other professions is what makes the sentence read as multi-profession, and dropping it would have been less protective, not more neutral |
| What was not changed | The selection default (still "none"), the mandatory documents folder, the memory budget mechanics (`fit_history`, whole exchanges, oldest dropped first), and the follow-up query heuristic. None of these caused the regression |

## Settled by the Data Folder UI session (28 September 2026)

Frontend follow-up to the 28 September Rust plumbing session (`Settings.data_folder`,
`choose_data_folder`, `ensure_suggested_data_folder`, `reveal_data_folder`). A working session note
had recommended starting with a settings-panel-only card and adding a sidebar presence "only once
the CSV/XLSX feature has something to show" — that was one agent's own recommendation, never
entered here, and the owner overruled it the same day.

| Subject | Decision |
| --- | --- |
| Where the Data Folder card lives | **Both.** A dedicated card in the sidebar, directly under the documents folder card, and the same card again in the settings panel — not settings-only. The owner's reasoning: the folder is choosable today (Rust plumbing already ships it) and a control that only exists in Settings reads as hidden, not as "not ready yet" |
| The Data Folder card's shape | Deliberately **smaller** than `WorkFolderCard`, not a clone of its behaviour: choose, see the suggested path, create it, change it, reveal it in the file manager. No Analyse button, no file listing, no index reset — there is no spreadsheet parsing or indexing to drive them yet (sprint 2b proper). Same CSS classes as the documents folder card (`.card`, `.card__title`, `.work-folder__row`, the same buttons), so the two look alike wherever a control is actually offered |
| CSV/XLSX in document retrieval | **Still out**, unchanged from the sprint 2 architectural reframe: tabular files get their own engine (`docs/SPRINT-2-ASSESSMENT.md`), not a flattening into text chunks fed to the documents pipeline. The data folder card added this session is infrastructure only — choosing and revealing a folder — and does not read, index or analyse a single file |
| Card title casing | "Data folder" / "Dossier des données", the same sentence-case convention as `workFolder.title` ("Documents folder" / "Dossier des documents"). Both render in capitals on screen because `.card__title` already applies `text-transform: uppercase` — the catalogue strings stay sentence case so the CSS rule, not a hand-typed literal, is what makes them read as capitals |
| `abbreviateWorkFolderPath()` | Renamed to `abbreviateFolderPath()` in `lib/folderPath.ts`: the function was already generic over any folder path, but its name named the work-folder concept specifically, and it is now shared by both cards. Behaviour and every Documents Folder call site are unchanged |
| Subfolder name casing (`DOCS`/`DATA` → `Docs`/`Data`) | **Changed the same day, after seeing both cards on screen.** `DOCUMENTS_SUBFOLDER_NAME` and `DATA_SUBFOLDER_NAME` (`work_folder.rs`) go from all-caps to one-capital title case, matching the card titles ("Documents folder" / "Data folder"), which already render in capitals through `.card__title`'s `text-transform: uppercase` — so the folder name on disk no longer needs to shout to read as prominent on screen. Applies to what a **new** folder is suggested and created as; nothing renames a folder that was already created as `DOCS`/`DATA` on an existing install, the same "never rename without an explicit act" rule as everywhere else in this product. No migration needed: `Settings.work_folder`/`data_folder` store the chosen absolute path as a string, not a name reconstructed from the constant, so an already-saved settings.json is unaffected either way |

## Settled by the tabular engine session (28 September 2026)

Sprint 2b session 4, on top of session 3's `TabularDataSource`/`TabularInventory`
(`docs/SESSION-DATA-04-TABULAR-Engine.md`). Code: `tabular::engine`, `tabular::structural`,
`tabular::question`, `tabular::escalation`, and `sheet_names` on `analysis_scope::ScopeEntry`.

| Subject | Decision |
| --- | --- |
| An aggregate that would have to read a formula column's values | **Refused outright** (`NotAnswerableReason::FormulaCannotBeVerified`), for `sum`, `min`, `max`, `distinct`, `group sum`, `sort` and the equality/comparison side of `filter` alike. A sum that silently mixed nine verified numbers with one unverified cached formula value would still look like an ordinary `Computed` total; refusing the whole aggregate is the only way `derivation: Computed` keeps meaning "the engine actually calculated this everywhere it says it did" |
| A plain row lookup (`row_at`, `largest_row`'s own row, a `filter`/`sort` result row) touching a formula cell | **Shown**, not refused - the cached value a person would see if they opened the file, via `tabular::inventory::text_value` preferring `cached_value` over the raw expression. A row listing is not an aggregate presented as verified, and hiding the cell would make the row listing itself dishonest about what is in the sheet |
| `sheet_names` on `ScopeEntry` | Carries sheet names, not indices - stable across a column being inserted elsewhere in the file. Enforced by making a sheet outside the restriction **indistinguishable** from a sheet that does not exist at all (`NotAnswerableReason::SheetNotFound` either way, and the sheets listed back on a refusal are only ever the reachable ones): a restriction a caller could learn to probe around would not be a real boundary |
| Natural-language classification of `filter`, `count`-with-a-filter, and `group sum` | **Out of `tabular::question`'s scope**, deliberately. The engine fully supports all three (`Operation::Filter`, `Operation::Count { filter: Some(_) }`, `Operation::GroupSum`); the classifier does not parse a filter clause or a group-by pair out of free text, because a wrongly parsed value is exactly the guess this pipeline exists to refuse. A question containing a filter clause still resolves its *recognised* part (a plain count, for instance) rather than failing outright - the clause is ignored, not mis-parsed |
| Wiring the engine into `commands::sourced_answer` / a chat surface | **Not done this session.** Tier 2 of the grounding priority chain (`tabular::escalation::TABULAR_INSTRUCTION`, guarded by the same neutrality test as tiers 1 and 3) is built and tested in isolation, but no Tauri command reaches `tabular::engine`, `tabular::question` or the Data Folder card yet, and how a document selection and a tabular selection combine in one conversation is still the open question `docs/SELECTION-AND-MEMORY.md` calls "documents and tables together". Building that routing under this session's own pressure would have meant deciding it without the scrutiny it is explicitly reserved for |
| A workbook re-read for an operation | **Re-hashed and rejected on mismatch** (`tabular::load_current`, `TabularError::WorkbookChanged`), the same "changed file, invalidated" guarantee `AnalysisScope::resolve`'s `missing`/`changed` reporting already gives a *selected* file, applied again at the moment the engine actually opens the bytes - a cached inventory built minutes earlier is never trusted without re-confirming the workbook it describes is still the one on disk |

## Settled by the tabular UI session (28 September 2026)

Sprint 2b session 5 (`docs/SESSION-DATA-05-TABULAR-UI.md`): the Data Folder card, cloned from the
documents folder card, and tier 2 of the grounding priority chain wired into `ask_with_sources`. Code:
`data_folder`, `tabular_answer`, `AnalysisScope::data_mode`, `components/DataFolderCard.tsx`,
`components/FolderCardParts.tsx`, `lib/tabularAnswer.ts`. Supersedes the Data Folder UI session's
"deliberately smaller" card.

| Subject | Decision |
| --- | --- |
| Is a data folder required? | **No**, superseding the 27 September line "the future data folder follows the same rule". What is cloned from the documents folder is the mechanism (suggested path, one-click create, `WorkFolderPolicy` validation), never the requirement. Nothing disables the composer for lacking one: the pilot has no spreadsheets (`docs/PILOT-GP.md`) |
| A tabular question the engine cannot answer exactly | **A nudge, built locally, never a bare code and never open chat.** An unclassified question, and any `NOT_DETERMINISTICALLY_ANSWERABLE` outcome, return `TabularAnswer::Nudge`: the engine's reason (or none, for an unrecognised question), the workbook's real sheets and columns, and one numeric formula-free column as a concrete example. The interface writes "its columns are …, try a sum, a minimum …, for example …". No model call. A second nudge in a row says so and asks her to name a column exactly; nudges are left out of the conversation memory sent to the model, like "analyse first" |
| The two inventories | **Stay separate, in the owner's words: "we do not want to merge."** `WorkFolderInventory` is unchanged and never learns about `tabular_inventories`. `IndexStore::all_tabular_inventories` is a new, tabular-only lookup. `data_folder::DataFolder` is the one place both are read, and it builds the Data Folder's `WorkFolderInventory` **with no index**, so a document row can never be joined onto a workbook sharing its relative path |
| Green or red | **Three states, like documents.** Green when at least one sheet has 8 or more data rows, 2 or more columns, and formula cells at most 5 % of its rows (`SheetInventory::looks_tabular`, numbers from LocalGridMind's `stats.py`, no code shared). Red when analysed and no sheet passes, when the adapter could read the bytes but not open a workbook (stored as an inventory with no sheet), or when the file is not a spreadsheet. Orange otherwise. Needed a new `SheetInventory::formula_cells` count, taken in the same build pass (`#[serde(default)]`, and every Analyse rebuilds every workbook) |
| LocalGridMind's whole-file formula budget (20) | **Not adopted.** It is the per-file gate the brief rules out: the engine already refuses column by column, and a workbook with one clean sheet beside a formula-laden one stays green |
| One combined scope | `AnalysisScope` carries **two** modes: `mode` (documents, unchanged) and `dataMode` (workbooks, relative to the data folder, `#[serde(default)]` = none). Not one mixed list: the same relative path can exist in both folders, and "tous" means the whole of one folder. The interface keeps the two lists apart and joins them only when a question is sent (`combineScopes`) |
| Routing | `AnalysisScope::tier`, before anything is read: documents chosen → tier 1 (or 3), unchanged; tables chosen and no document → tier 2, `tabular_answer::answer`, no gateway call; both → **refused** with `documents_and_tables_together`, never silently routed. "Tous" in the data list keeps every green workbook; several selected and none named → `which_workbook`, never a guess; a red or unanalysed workbook is refused again in Rust even if the selection holds it |
| "Ask the AI" on a tabular answer | **Not offered.** Regenerating returns the same answer, and there is no model on this tier to ask instead. `TABULAR_INSTRUCTION` stays unsent until documents and tables are combined |
| Citations | A tabular answer cites `file, feuille <sheet>[, colonne <column>]` in the same Sources disclosure a document answer uses for its pages |
| Data selection wording | "Données utilisées : aucune / toutes", agreeing with *données*, rather than the brief's "aucun / tous" copied from the documents card |
| `TabularValue` on the wire | **Adjacently tagged** (`{"kind": "sum", "value": 12.5}`). Internally tagged, as session 4 left it, it could not serialise a bare number and every computed value would have failed at the bridge; found by the first test that crossed it |
| Group questions ("which agency costs the most", "total amount per agency") | **Now classified**, superseding the 28 September engine session's "group sum out of `tabular::question`'s scope" for group-by only (filter clauses stay out). Raised by the owner after LocalGridMind answered this at once and this app returned a nudge. Safe for the same reason the engine is: the question must name a real group column as whole words (`sub_agency` needs "sub agency", never "agency" alone; a two-word group column may be named by its last word when that word is unique, "year" for `calendar_year`), and no value is parsed. "which/what … most/highest/largest" → `Operation::LargestGroup` (top five totals, largest first, ties reported as ties); "sum/total … per/by" → `Operation::GroupSum`. The answer says every time that it is a **total per group**, not the largest single row |
| The column to total, when the question names none | Chosen, never guessed: numeric, formula-free, not a year (every value a whole number 1900-2100, `ColumnInventory::year_like`) and no identifier, code or date word in its name; among those, the one whose name reads as an amount. Several equally good → `which_measure`, asking her to name one. The name words are locale data (`column_names` in each pattern pack), read from **every** pack at once, because a workbook's headers are in the language it was exported in - LocalGridMind's `_AMOUNT_HINTS` / `_ID_NAMES` idea, no code shared |
| "List the agencies" | **The column's different values** (`Operation::Distinct`), reached by a new `list` word set ("list", "show", "liste", "affiche"...) beside the existing "distinct / unique". Column names now also match a regular plural as a whole word ("agencies" for `agency`, "-aux" for "-al") when the older substring match finds nothing. Long lists show fifty values and count the rest |
| Question language | The interface language's words are tried first, then the other packs'. An English question on a French interface is read; each pack still points only at the workbook's real columns |
| The nudge's examples | Built by the same column choice (never `calendar_year` again), with a second, group example when the sheet has a text column. A Rust test fills the catalogue's own example sentences with the nudge's columns and requires each to be answered - an example that earns a second nudge fails the build |
| Data Folder Analyse | Renames with `filename_sanitizer` first, same rule and same log as documents, then parses every CSV/XLS/XLSX/XLSM, caches each inventory and drops those of files no longer present. Its own `useIndexing` instance and its own `FileHashCache`, so neither folder's pass or reset touches the other's |

## Settled for the tabular continuation, session 7 (30 September 2026)

Owner's answers, given directly in chat against `docs/SESSION-DATA-06-Findings.md`. Full reasoning and two
corrected misreadings: `docs/SESSION-DATA-07-Proposals.md`. Supersedes, where they conflict, the 28
September tabular engine session's "filter clause is ignored, not mis-parsed" and the roadmap's
"nine operations" line.

| Subject | Decision |
| --- | --- |
| A filter clause the classifier does not apply | **Never the unfiltered value, unconditionally.** Returns a nudge, reason `filter_not_supported`, naming the detected column and value - replacing, not sitting beside, `filter_and_group_sum_are_deliberately_out_of_this_classifiers_scope`'s current unfiltered count. Once the model-assisted path below exists, such a question is tried there first |
| French and English numbers in one column | **Read per column, never per cell**, the same consistency-scoring idea `csv_adapter::detect_delimiter` already uses between `,` and `;`. A comma followed by exactly three digits in every cell that has one is corroborated as a thousands grouping; a comma not shaped that way anywhere in the column is corroborated as a decimal separator. Consistent evidence one way wins; no evidence either way falls back to the interface locale (decimal comma for `fr-FR`); genuinely mixed evidence is reported, never guessed. `€`, `$`, `%` suffixes and a space or no-break space as a thousands separator are read as already drafted for this decision |
| Dates, as text or as a workbook's own date cells | **Parsed to one real comparable calendar value**, not compared as text and not left as a raw serial number - the single fix behind two bugs found live: a `dd/mm/yyyy` text column sorting by day-of-month, and an XLSX date cell displayed as `"46090"`. A pure-Rust date crate already sits in `Cargo.lock` transitively and builds on both mandatory platforms |
| Recommended format when the product asks her to produce or clean up a date column | **ISO 8601 (`yyyy-mm-dd`)** - the actual international standard for a text date, because it sorts correctly as plain text with no locale ambiguity. A recommendation for wherever the product writes a date column back out (a CSV export, if built), not a substitute for parsing the formats a workbook already carries |
| The roadmap's operation count | **Not a cap being exceeded improperly.** The engine already has eleven operations by two prior, already-recorded decisions (`LargestGroup`, `RowAt`); `docs/ROADMAP.md`'s "nine" was a stale number in a document, not a limit the code overran. The risk line is reworded to name the real guardrail - a new operation is added by an owner decision recorded here, never quietly - instead of a count that goes stale each time one is added |
| Whether the model may help understand a question the classifier cannot | **Yes.** The model reads the workbook's schema only - sheet names, column names, types, row counts, never a cell value, never a row - and returns a JSON query plan in a closed vocabulary; Rust validates the plan against the real workbook and executes it with the existing engine; the number in the answer is always the engine's arithmetic. New provenance, "interpreted by the model", is shown beside "computed". A gateway that is unreachable degrades to the existing nudge: the model widens which questions get an answer, and is never load-bearing for any answer's correctness. One DPIA line: column and sheet names, never values, reach the gateway - which on the pilot's deployment is Ollama on her own Mac mini, not a third party |
| What `tabular::escalation` may show the model, resolved with the line above | **An already-computed result** - a group's total, a column's distinct values, a single looked-up row's cells - may reach the model as evidence, because the concern this product's rules protect is data leaving the workstation or being **stored** outside her control, not a locally running model transiently reading a value while writing an answer. Nothing here is written to disk, a log or a database by the server or the model, unchanged from `AGENTS.md`'s existing rule. What stays forbidden is a raw, unaggregated row list (`TabularValue::Rows`) - the "whole workbook, just in case" shape this pipeline exists to refuse - not a single already-computed fact |
| A CSV export offered for a workbook the engine handles poorly | **Deferred, not scheduled into sessions 9-16.** Would help where a workbook's *shape* is the obstacle (a title row above the header, an all-formula sheet, inconsistent sheets) written out flat with ISO dates and one stated number format; would not help gaps that are classifier limits (a filter clause, a per-group count), which recur identically in a CSV. If built, it is a suggested, approved, logged write into the Data Folder under the product's existing "Export" concept (`AGENTS.md` item 3), never a silent one - and it is worth checking first against `AGENTS.md`'s frozen "DOCX-to-PDF or any other document conversion" line, which was written about document extraction, not a tabular-only export, but deserves an explicit look before it is scheduled |
| The gateway test double used to prove a deterministic path makes zero calls | **Kept**, reworded only. Not a "fake AI" in the product - a test-only HTTP stand-in that fails any request it receives, the same technique this repository's own `tests/chat_idle_timeout.rs` and `tests/chat_cancellation.rs` already use for other gateway behaviour. `docs/ROADMAP.md`'s wording changes to "a test double that fails any request" so it cannot be misread as describing product behaviour |
| D1, implemented | **Done, session 9 (30 September 2026).** `tabular::question::classify_traced` reports the question's residual words; `tabular_answer::detect_filter` checks them against the reachable sheet's real cell values, folded, and against a plain number, before the engine ever runs. A match returns `TabularAnswer::Nudge` with the new machine code `filter_not_supported` and the detected column/value, never the unfiltered count or sum. Session 11 still owes the filtered value itself; this session only stops the dishonest one |
| D2, implemented | **Done, session 10 (30 September 2026).** One parser, `tabular::inventory::number_shape` plus `resolve_numeric_column`, used by the inventory to type a column and by the engine to compute over it, so the two can never disagree. Decimal point or comma; a thousands mark (space, U+00A0, U+202F, point or comma) accepted only in groups of exactly three digits and never the same character as the decimal mark; a currency or percent sign before or after, with or without a space, kept as `ColumnInventory.unit`; negatives with `-` or parentheses. A lone separator with exactly three digits after it (`1,234`) cannot be read from one cell alone: another cell of the same column that is unambiguous by shape settles it; two or more equally ambiguous cells sharing that shape corroborate a thousands grouping on their own (`depenses.csv`'s fixture); a separator with contradictory evidence in the same column is never guessed; only a lone ambiguous cell with nothing else in the column falls back to the interface locale (decimal comma for `fr-FR`). A column is `Numeric` at 95% parse or better; `unparsed_count` keeps what did not parse, shown beside a sum rather than silently dropped |
| D3, implemented | **Done, session 10 (30 September 2026).** `chrono` 0.4.45 - already resolved transitively, now a direct dependency - plus calamine's own `dates` feature (also chrono, no second date library) for XLSX cells: `CellValue::Date` holds a real `chrono::NaiveDate` rather than the raw serial number the file stores. Accepts `dd/mm/yyyy`, `dd-mm-yyyy`, `dd.mm.yyyy` and ISO `yyyy-mm-dd` text, checked against a real calendar (`NaiveDate::from_ymd_opt`), not merely plausible ranges. A day-first column types `Date` only when some cell's day exceeds 12, or a cell is ISO, or a cell is a real date cell from the file format - any one of those settles the reading for the whole column; with none, the column stays `Categorical` and carries `ambiguous_date: true` rather than guessing which of `dd/mm` or `mm/dd` the file meant. Weekday, month, year and range filters stay out, owed to session 11 |
| D4, implemented | **Done, session 10 (30 September 2026).** `detect_header_row` scores every candidate in the first 20 rows - how much of the sheet's widest row it fills, how many of its own labels are distinct, how much of the row beneath it reads as typed data rather than more text - and keeps the highest scorer, rather than accepting the first all-text row. A row with fewer than two filled cells is never a header outright, which is what now lets `factures.csv`'s title row (`Export du 12/03/2026`) be skipped in favour of the real header beneath it and the blank row between them |
| D5 | **Accepted as proposed, 30 September 2026** (`docs/SESSION-DATA-07-Proposals.md`): the engine gains `mean`, `median`, `least_group`, `top_n` (N capped at 50, a larger N stated and capped rather than refused) and `count_per_group`, each also accepting an optional filter (session 11's `filters: Vec<FilterSpec>`, built for exactly this). This was a request to fix a stale "nine operations" number in `docs/ROADMAP.md` - already reworded to name the real guardrail, a new operation needs an owner decision recorded here - not a cap on the five being proposed. **Implemented, session 12 (1 October 2026).** `tabular::engine::Operation::{Mean, Median, LeastGroup, TopGroups, CountPerGroup}`, with `TabularValue`'s matching variants (`TopGroups` also carrying `requested` and `capped`) and new classifier vocabulary (`groups.least`, `groups.top`, `operations.mean`/`median`, and a `numbers` word list for a `top_n` question's N). Found and fixed along the way: `tabular::question::detect_structural`'s row-count branch previously swallowed "how many rows per agency" as a plain row count, because `per` was already-recognised vocabulary and left nothing residual to defer to `Count` - it now checks for `groups.per` first, the same way the `sheets` check already excludes `rows`/`columns`. Every gap E reference case passes in both languages (`tests/tabular_reference_cases.json`), plus four new adversarial scenarios: an odd count combined with a session 11 filter, a request beyond the 50 cap, `count_per_group` over all 2,400 rows of `agencies.csv`, and `count_per_group` with a filter |

## Settled by the deterministic filters and dates session, session 11 (30 September 2026)

Gaps A and B (`docs/SESSION-DATA-REFERENCE-report.md`), closed together because both are "a residual
word anchored on real data becomes a filter" - A for an equality value, B for a date. Code:
`tabular::engine` (`Comparison`, `FilterSpec`, `AppliedFilter`, every `Operation` but `Filter` and
`RowAt` now carrying `filters: Vec<FilterSpec>`), `tabular::inventory::resolve_date_column`,
`tabular::question::filter_words`, `tabular_answer::detect_filters`. Every gap A and B reference
case now passes in French and English (`tests/tabular_reference_cases.json`); gap E (mean, median,
least group, top N, count per group) is untouched, owed to session 12.

The filter grammar - what is recognised, what is asked, what is refused:

| A residual word or phrase | Recognised as | Guard |
| --- | --- | --- |
| A value found, whole word, in exactly one reachable column | `Comparison::Equals`, the column's own full value (never a lone word of a multi-word value) | Found in two columns → `TabularAnswer::WhichColumn`, asked, never picked. The question may already have named the column ("where fournisseur is Alpha"); that resolves the ambiguity deterministically rather than asking |
| A weekday or month-name word (pack vocabulary, `filter_words`) | `Comparison::Weekday`/`Comparison::Month` against the single reachable `Date`-typed column | More than one reachable `Date` column, or none, and the word is not recognised as a filter at all - never guessed |
| A four-digit year | `Comparison::Year` against the single reachable `Date`-typed column, when equality does not already resolve it against a numeric year-like column (agencies.csv's `year` is `Numeric`, not `Date` - the ordinary equality path reads it) | Same as weekday/month: more than one candidate, or none, and it is not read as a filter |
| "between X and Y" (pack `between` word) with two real dates in the question's own text | `Comparison::DateRange`, inclusive both ends, against the single reachable `Date` column | Extracted from the question's raw text, not from residual tokens split on `/` - a reconstruction from separated digits would have to guess which triplet paired with which |
| A comparison word (pack `comparisons.greater_than`/`less_than`) beside a number | `Comparison::GreaterThan`/`LessThan` against the operation's own column when it is numeric, else the single reachable numeric column | No number nearby, or more than one numeric column with none implied: not read as a filter (this is what stops "la salle qui a le moins de duree_min", a superlative group phrase gap E still owes, from being misread as a numeric filter) |
| An unmatched word that looks like an attempted value (capitalised, and not the question's own first word - a sentence's opening capital is grammar, not a proper noun) | `NotAnswerableReason::ValueNotFound`, with up to five close real values (folded prefix or edit distance ≤ 2) | Never an empty result presented as zero. A lowercase leftover word ("invoices", a pack's filler list is grammar, not a domain dictionary) is silently ignored, as before |
| A filter naming a column typed `Numeric`/`Date` the wrong way round for its comparison, or a column carrying a formula | `NotAnswerableReason::NonNumericColumn`/`NonDateColumn`/`FormulaCannotBeVerified` | Same refusal an aggregate over the wrong type already gave; a filter is never applied to a column it could not honestly be computed from |
| Several recognised filters in one question ("Alpha in 2025") | Combined with **AND only** | No OR, no nesting - forbidden outright, per this session's anti-pattern list |

| Subject | Decision |
| --- | --- |
| A row-count question with a leftover word ("how many rows does Harbor have") | Deferred from `StructuralQuestion::RowCount` to the ordinary `Count` operation whenever something survives `residual_words` beyond the sheet and column it already resolved - `tabular::question` still never reads a cell value itself, but it can tell whether anything is left over, and hands that case to `tabular_answer::detect_filters`, which can. Fixed LocalGridMind limit 4 (`docs/SESSION-DATA-REFERENCE-report.md`) |
| `Sort` over a `Date`-typed column | Reads `inventory::resolve_date_column`, exactly as `Numeric` reads `resolve_numeric_column` - the engine session (28 September 2026) only ever wired the numeric branch, so a `dd/mm/yyyy` text column sorted as text, day of month first, for two sessions (live bug B1, fixed) |
| `TabularLocator.filters: Vec<AppliedFilter>` | Every filtered value's own filters, described as data (`Equals`, `Weekday`, `Month`, `Year`, `DateRange`, `GreaterThan`, `LessThan`, `Between`, `Contains`, `In` - one to one with `Comparison`) - never prose. The interface's "Understood as" line is built from this and the interface locale, the same split every other machine-code-in/sentence-out boundary in this product already keeps |
| A value found in two reachable columns | `TabularAnswer::WhichColumn { value, candidates }`, a new answer kind - not folded into `Nudge`, because it is not a refusal: the engine could compute either reading, and picking one silently would be exactly the guess this pipeline exists to refuse |
| Weekday numbering | ISO: 1 = Monday .. 7 = Sunday, computed from `chrono::Weekday::number_from_monday`, never a locale-dependent week start |
| Filter-word vocabulary (`comparisons`, `weekdays`, `months`, `between`) | Read from the question's own locale pack only (`question::filter_words`, the same `pack_for` fallback chain `classify` uses), **not** merged across every shipped pack the way `column_names` are. A workbook's headers are the export's language; a filter word is the question's own |
| Numeric `Comparison::Between` | Defined on the engine, alongside the four date comparisons, but `detect_filters` does not yet build one from residual words - only the date range does. Deferred, not scheduled |

## Settled by the typed column cache session, session 13 (1 October 2026)

Measured first (`docs/SESSION-DATA-13-Column-Cache.md`), on the development PC:
`cargo test --release --test tabular_column_cache_bench -- --ignored --nocapture`, recorded in
`docs/HARDWARE.md`. A 100,000-row, 8-column CSV costs 819 ms per question; a 50,000-row XLSX, 1.52 s -
both well over the session's 300 ms budget on the faster of the two machines this product ships to, so
the cache was built. The 2019 practice PC's own number is still owed (`docs/HARDWARE.md`).

| Subject | Decision |
| --- | --- |
| What is cached | The whole parsed `Workbook` (`tabular::CellValue` rows - numbers and real calendar dates for XLSX, decoded text for CSV, exactly what an adapter already produces), not a column-major re-encoding. `tabular::engine` already re-derives numeric/date values from these cells on every call (`inventory::resolve_numeric_column`); session 6's own measurement found that step about 6 % of a question's cost, so rebuilding `engine.rs` around a second, pre-parsed representation would have spent real risk on an already-cheap part, against `AGENTS.md`'s "do not rewrite what works" |
| Where it lives | `IndexStore`'s own SQLite file (`tabular_workbooks`, beside `tabular_inventories`), never a file beside the source workbook - the existing store, no second database, per the session's anti-patterns |
| The key | The content SHA-256 (`workbook_id`), not the path - unlike `tabular_inventories`. The same bytes under two names (a rename, `docs/WORK-FOLDER-INVENTORY.md`'s clean file names) share one cache entry, and a changed file is a different key rather than a row to overwrite |
| The fast validity check | Not reimplemented. `inventory.workbook_id`, by the time `tabular_answer::answer` sees it, has already passed `FileHashCache`'s own size-and-modified-time check (`DataFolder::discover`, every question) - trusted only when both still match, a full read and rehash otherwise. The typed cache is consulted by that same hash; a miss falls back to `tabular::load_current` exactly as before, and `WorkbookChanged` still fires exactly as before (`load_workbook_cached`, `tabular_answer.rs`) |
| Residual risk accepted | A file overwritten with different content of the exact same byte length, whose modified time is then set back by hand, reads as unchanged and can serve a stale cached value - the same risk `FileHashCache` already carries for the Documents pipeline (`docs/WORK-FOLDER-INVENTORY.md`, "Known cost"), not a new one. Documented and reproduced in `tabular_answer::tests::an_overwrite_with_the_same_size_and_a_restored_modified_time_can_serve_a_stale_cached_value`. Hashing on every question was the alternative this session offered and was not taken: it would have erased the measured gain entirely |
| Lifecycle | Cleared wholesale (`IndexStore::clear_tabular_workbooks`) at the start of every `data_folder::analyse` pass - covers a changed file, a removed file and a freshly reanalysed one in the one place that already rebuilds `tabular_inventories` unconditionally - and from the Data Folder card's Reset (`commands::reset_data_index`), beside `clear_tabular_inventories`. Nothing is ever written for a red or unanalysed workbook: the cache is only populated from the `Operation` route's own success path, which a red or unanalysed file never reaches |
| Locale safety | A cache hit still rebuilds `TabularInventory` from the cached cells with the question's current locale (in memory only, no read) rather than reusing a stale-locale inventory - the same guarantee `tabular::load_current` already gave by always re-deriving a "fresh" inventory, kept exactly, not loosened for speed |

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
| Changing the work folder and what happens to the index | Choosing a different folder does **not** empty the index — and before 24 September 2026 that meant the previous folder's passages stayed citable for ever. They are now dropped by the first pass over the new folder. **Consequence:** after switching folders the index still describes the old one until she presses Analyse. Since 27 September 2026 that no longer reaches an answer: the selection starts empty after a folder change, and "tous" searches only files present in the current folder. Deleting `settings.json` by hand therefore loses no analysis: re-picking the same folder finds the index exactly as it was |
| Dropping index rows for a file removed from the folder | **Done, 24 September 2026**, at the start of every pass. Until then a deleted document's rows outlived it: the panel stopped listing it, because that reads the filesystem, while retrieval went on offering its passages as evidence for an answer. The pass names what it forgot, because only she can tell a deliberate deletion from a folder that failed to mount |

## Out of scope until the pilot holds

Fine-tuning, mobile applications, a multi-practice hosted service, autonomous overnight operation, a cloud
fallback model on real files, a server-side case index, irreversible actions without review, a second shared
profession, wide-area multi-site access outside a practice VPN.

Asking a second profession questions is not opening one. Discovery findings wait in `docs/DISCOVERY.md` until
the GP pilot holds.
