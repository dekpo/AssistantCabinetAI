# Roadmap — v0 sprint

**Revised 16 September 2026.** Replaces the earlier ten-phase plan (mapping table at the end).
Canonical direction: `docs/BRIEF-V0-PROTOTYPE.md`. Sprint rule: `.cursor/rules/v0-sprint.mdc`.

**Amended 18 September 2026** after the Sprint 2 architectural reframe: Sprint 2 splits into 2a
(documents, carries milestone A) and 2b (tabular data). Reasoning and the full assessment:
`docs/SPRINT-2-ASSESSMENT.md`. Long-term direction, which is deliberately **not** in this file:
`docs/PLATFORM-VISION.md`.

One goal: an **installable** prototype the pilot GP can use, which answers **with its sources** on her own
documents. Given a choice between one more feature and a more reliable, private, testable and replaceable
flow, take the second.

The chain to make hold end to end:

```text
She opens Assistant Cabinet AI
  → she picks a folder
  → the documents are indexed on her workstation
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

## Sprint 2a — local document retrieval (24 → 30 September)

Deliverable: the chain above, on the fictional files, with citations. This sprint carries milestone A,
so nothing else may be added to it.

- Extraction on the workstation: native PDF, DOCX, TXT, MD. Chunking that **keeps** file, page and section.
- File discovery over the work folder, extension-driven, symbolic links refused. The same mechanism must
  later serve a legal or accounting practice: no GP-specific loader, no assumption about folder contents.
- Local index (SQLite: full-text search plus vectors) in `%LOCALAPPDATA%` (`app_local_data_dir()`), behind
  a replaceable interface. Not the roaming `%APPDATA%` that holds `settings.json`: the index contains
  document text, and a roaming profile copies `%APPDATA%` to a server. Neither the database nor the
  embedding model may show through into business code.
- Embeddings: first through a **no-store** call to the gateway, which needs `POST /v1/embeddings` — the
  route does not exist yet, although `AIProvider.embed` does. A local ONNX computation stays possible
  later behind the same interface, without touching anything else.
- Send only the selected excerpts, with a size cap. Never the whole folder "just in case".
- An answer that cites file, page and passage, through the common `Source` model in
  `docs/ARCHITECTURE.md`. Without sufficient excerpts, the product says it did not find enough
  information rather than generating one.
- **Tests:** retrieval, source attribution, refusal to answer beyond the sources, and a test that
  **proves** the server holds no file, no chunk and no text in a database or a log after a request.
- Extend the sandbox: it currently holds only `.txt`. It needs fictional native-text PDFs, a DOCX, and one
  scanned image PDF reserved for the OCR contract.

**Milestone A acceptance, 30 September:** on a folder of fictional files, three questions give three
sourced answers, a fourth off-topic question gets the refusal, and the isolation tests pass.

## Sprint 3 — GP workflows (1 → 7 October)

Deliverable: the actions that cost her 1.5 to 2 hours a day. Three flows, not five.

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
| Scanned PDFs (OCR) | **Out** of v0: report empty extraction and refuse to classify rather than guess |
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
| 5 Document extraction | Sprint 2a, without scan OCR |
| 6 Connect the window to the gateway | Sprint 1 |
| 7 Retrieval on the client | Sprint 2a — **moved up**, it carries milestone A |
| — (new) Tabular data | Sprint 2b, after milestone B |
| 8 Mac mini port | Sprint 4, at the practice |
| 9 Field pilot | After 14 October |
| 10 More professions and agents | Unchanged: after real validation |

## After 14 October, not before

Sprint 2b (tabular data) comes first, then voice, vision, scan OCR, mobile, app stores, a second
profession, autonomous agents, fine-tuning, an accounting module, the Ameli account, automatic
transmission, RBAC, billing, analytics.

Where that ordering is heading, without dates: `docs/PLATFORM-VISION.md`.
