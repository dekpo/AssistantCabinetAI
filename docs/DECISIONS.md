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

## Out of scope until the pilot holds

Fine-tuning, mobile applications, a multi-practice hosted service, autonomous overnight operation, a cloud
fallback model on real files, a server-side case index, irreversible actions without review, a second shared
profession, wide-area multi-site access outside a practice VPN.

Asking a second profession questions is not opening one. Discovery findings wait in `docs/DISCOVERY.md` until
the GP pilot holds.
