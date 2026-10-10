# Architecture

Merges the earlier French notes on the target architecture, agent runtimes and client bridges.

## Overview

```text
Usual software (Word, document management, Explorer, VS Code / Continue, ...)
  └─ adapters (pane, add-in, OpenAI-compatible configuration)
Assistant Cabinet AI (native window: work folder + plan + chat + local index)
Open WebUI (owner workbench, no sensitive files)
           │
           │  LAN + TLS
           ▼
Gateway — apps/server (Python, FastAPI, Pydantic)
  ├─ Auth (per-person keys, later LDAP / Entra)
  ├─ Model aliases, queue, quotas
  ├─ no-store (never writes excerpts or prompts)
  ├─ Tool catalogue and plan validation
  └─ Request register (metadata only)
           │
           ▼
Ollama (on the server's 127.0.0.1)  ←  Hugging Face / Ollama library weights
```

Ollama and Open WebUI are an inference substrate and a UI. They run in **Docker Compose**, the same
definition on the development PC and on the Mac mini. The **product** is the gateway, the native client and
the approved plans. Business retrieval lives on the workstation, not on the server.
See `docs/OPERATIONS.md`, `docs/RETRIEVAL.md`, `docs/CLIENT.md`.

## Layers

| Layer | Responsibility | Starting technology |
| --- | --- | --- |
| Environment | Pinned versions, Windows to Mac portability | Docker Compose (`docs/OPERATIONS.md`) |
| Inference | Chat, embeddings without retention | Ollama behind the gateway |
| Gateway | Auth, aliases, quotas, no-store, metadata audit, `AIProvider` | `apps/server`: Python, FastAPI, Pydantic |
| Admin UI | Accounts, demos, **non-sensitive** corpora | Open WebUI (owner workbench, frozen for v0) |
| Practice client | Chat, local index, plans, history | `apps/desktop`: Tauri 2, React, TypeScript |
| Retrieval | Full-text index plus vectors **on the client** | SQLite in the Rust core (`docs/RETRIEVAL.md`) |
| Tabular analysis | Workbook inventory plus deterministic lookup, **on the client**, working with the model off | `calamine` and `csv` in the Rust core (`docs/RETRIEVAL.md`) |
| Extraction | Native PDF, DOCX, TXT, MD on the workstation | `pdf-extract` and `quick-xml` in the Rust core |
| OCR | Scanned PDF, JPEG, PNG read **locally**, never in the cloud and never on the server | One local engine behind `OcrProvider` (`docs/SPRINT-2.5-ASSESSMENT.md`) |
| File tools | list / extract / propose / apply | JSON catalogue plus an MCP façade |
| Security | VLAN, TLS, scopes, disk encryption | `docs/PRIVACY-AND-SECURITY.md` |

## Why a dedicated client and not only a chat

Chat matters, but it must not become the practice software. The client offers a pane over the folder
already open — drag and drop, plan, approval — **and** a chat. Other software connects through the same
gateway. Open WebUI is the owner's workbench, never the doctor's screen.

## Contracts to freeze early

1. OpenAI-compatible API (`/v1`) for every client.
2. A versioned tool catalogue (`tools/v1`).
3. An action-plan format (dry run, hashes, reason, revocable id).
4. Metadata audit (actor, timestamp, hashes, decision) — never the confidential content.
5. A no-store policy plus a path allow-list.
6. Model aliases (`cabinet-chat`, `cabinet-rapide`, ...): allow-list only, licence record mandatory. No open
   model store, no clinical-care weights in the pilot. See `docs/MODELS.md`.
7. One language variable, `locale`, owned by the client. See `docs/LANGUAGE-AND-LOCALE.md`.
8. One `Source` model for every kind of evidence, carrying how it was obtained. See below.
9. A verified fact and a model's reasoning are distinguishable in the data, not only in the wording.

## Workstation capabilities

The workstation owns the corpus, so it owns the capabilities that read it. Each one is a port; the v0
implementation behind it is deliberately boring and replaceable. None of these modules imports `tauri`:
the Tauri commands are a thin adapter over them, which is what keeps a second client possible later
(`docs/PLATFORM-VISION.md`).

```text
DocumentSource      discover(work_folder) -> DiscoveredFile[]     extension-driven, allow-list only
WorkFolderInventory discover(root, index) -> FileRecord[]         every file on disk, joined with the index
FilenameSanitizer   sanitize_folder(root) -> SanitizeReport      clean names on Analyse; never overwrites, always logged
FileReferenceResolver resolve(text) -> FileReferenceResolution    exact / ambiguous / no match, never a guess; accents and case folded
TextExtractor       extract(path) -> ExtractedDocument            pdf / docx / txt / md, plus OCR
OcrProvider         recognise(page image) -> OcrPage              one local engine, replaceable
Chunker             chunk(document) -> Chunk[]                    keeps file, page, section
Embedder            embed(text[]) -> Vector[]                     gateway today, local ONNX later
IndexStore          upsert / search_lexical / search_vector       SQLite today
RetrievalService    search(query, scope) -> Evidence[]
AnalysisScope       resolve(inventory) -> ScopeResolution         the documents she chose; default none, which answers without excerpts
ConversationMemory  fit_history(turns, budget) -> ChatTurn[]      recent exchanges within the model's window; excerpts never resent

TabularDataSource   open(path) -> Workbook                        csv, xlsx
TabularInventory    build(workbook) -> WorkbookInventory          structure and facts, full pass
InventoryStore      put / get_by_hash / invalidate                SQLite today
TabularAnalysis     analyze(query, scope) -> TabularOutcome       deterministic, no model
```

File discovery is generic. It recognises `.pdf`, `.docx`, `.txt`, `.md`, `.jpg`, `.png`, `.csv` and
`.xlsx` by extension and contains no assumption about what a folder holds, because the same mechanism
must serve a legal, accounting or notarial practice unchanged.

The inventory sits above discovery and below both content pipelines. It is the single authority for
**filesystem facts** - how many files there are, what they are called, which extension each carries,
where it sits, whether it was read and by which method - and those facts are never inferred from a
retrieved passage or written by a model. It holds metadata only: no document text enters a `FileRecord`.
`FileRecord` answers "which file is this?"; `Source` below answers "which evidence from it supports this
answer?". The same inventory and the same resolver will serve the tabular pipeline, which is why neither
carries a document-specific field. See `docs/WORK-FOLDER-INVENTORY.md`.

Spreadsheets are **not** flattened into text chunks to resemble PDFs. They get their own pipeline, their
own inventory and their own deterministic operations.

Reading a column's own locale (`docs/DECISIONS.md`, D2-D4):

- Decimal and thousands marks are read from a column's own cells, never the operating system's locale.
- A lone separator no single cell can settle is decided by the rest of its column, or by the interface
  locale only when the column offers no evidence either way - never guessed from one value.
- A currency or percent sign travels with the column as a unit; an unparseable cell is counted, never
  folded silently into a sum. A column is `Numeric` at 95% parse or better.
- A date is a real calendar value, never a serial number or a locale guess; a day-first column types
  `Date` only when some cell's day or format rules out the month-first reading, `Categorical` otherwise.
- The header row is the best-scoring candidate in the first twenty rows - fill, distinct labels, how
  differently the row beneath reads - never simply the first row of text.

## OCR is an extraction path, not a second pipeline

A scanned page is a document whose text has to be recovered before anything else can happen to it. That
recovery is an ingestion capability, so it sits inside `TextExtractor` and produces the same
`ExtractedPage` as a text layer does. Nothing downstream — chunking, the index, retrieval, citations, the
GP workflows — learns that OCR exists.

```text
page of a PDF
  ├─ usable text layer   -> ExtractedPage { origin: TextLayer }
  └─ no usable text      -> rasterise in memory -> OcrProvider -> ExtractedPage { origin: Ocr }

JPEG / PNG               -> OcrProvider -> a one-page ExtractedDocument
```

Four boundaries hold, and each is a test rather than an intention. Detection is **per page**, so a
born-digital page is never rasterised and a mixed document costs one OCR call per scanned page. The
engine runs **locally**, in memory: no cloud OCR service, no external document processor, no Internet
requirement, and no recognised text written anywhere but the local index. `OcrProvider` is the only OCR
symbol the application knows, so a better engine is one new file. And a page the engine could not read
produces **no chunk**, so there is no path from a failed recognition to a citation.

A workflow that asked whether its input was born-digital or scanned would be a workflow that has to
change again for the next input format. None of them asks.

## The common source model

Documents and workbooks differ internally and are identical at the boundary.

```text
Source {
  origin:     { relative_path, sha256, modified_at }
  locator:    Document { page, section, chunk_id, char_range }
       |      Tabular  { sheet, header_row, column, row_range }
  derivation: Extracted                                    copied from the file's own text
       |      Recognised { engine, confidence }             a machine read a picture of it
       |      Computed { operation, operands, row_count }   we calculated it
       |      InterpretedByModel { ..Computed, model_alias, plan }  a model chose the plan; we still calculated it
       |      FormulaStored { expression }                  the file says so; we did not verify it
       |      ModelAsserted                                 the model said it
}
```

`derivation` is the contract that keeps a verified fact apart from a generated one. It lets the interface
render a computed total differently from a model sentence, and it lets a test assert that nothing
labelled `Computed` ever passed through an LLM. A citation is never invented: an answer may only cite a
`Source` that the retrieval or analysis step actually returned.

`InterpretedByModel` is `Computed`'s sibling, not `ModelAsserted`'s: the model never states the number.
It names which question it understood - sheet, columns, filters, the operation - from the workbook's
schema alone, and Rust validates and runs that plan through the same engine `Computed` already uses
(`tabular::query_plan`, session 14's hidden interpreter, `docs/SESSION-DATA-14-Query-Plan.md`). The value
is always the engine's full-pass arithmetic; the model only ever widened which questions reach one.

`Recognised` is not a flavour of `Extracted`. "The letter says 6.8" and "a machine thinks the letter says
6.8" are different claims, and collapsing them would let OCR uncertainty arrive at the user as model
certainty. Keeping it as a field rather than as wording means the interface can mark an OCR-sourced
citation, and the confidence that came with it can gate what enters the index in the first place.

## The knowledge layer and its ports

The client keeps a local, source-grounded directory of the entities (people, organisations, places,
identifiers, items, terms) found in the files the user has analysed: `apps/desktop/src-tauri/src/knowledge/`,
persisted in the `kb_*` tables of the same `index.sqlite3`. It is candidate discovery inside the user's own
selection and a shared vocabulary; it is never an authority over a number and never a memory of
conversations (`docs/DECISIONS.md`, "Knowledge Base and the product direction"). Three seams are replaceable and
business code imports no engine behind them:

```text
EntityExtractor   a source -> Candidate names (a name as written, type, role, method, locator)
                  deterministic text extractor (lot 4), table extractor (lot 5); a local NER model or a
                  spaCy adapter would implement the same trait and sit beside them in a CompositeExtractor
EntityResolver    a name -> NewEntity | Existing | PossibleMatch | Ambiguous | Unresolved
                  DeterministicResolver today; it reads the store only through EntityLookup, so it is
                  tested against an in-memory fake and runs against StoreLookup
PhoneticEncoder   a word -> a key, chosen by locale (fr-rules, en-rules); calibrated on public name lists
```

What the words are comes from **lexicon packs**, JSON files bundled with the application
(`resources/knowledge/base` always, `resources/knowledge/packs/<id>` when active), loaded and validated by
`knowledge::packs`. They hold titles, particles, stop-words, organisation markers, identifier patterns, header
vocabulary and role label keys - words and patterns, never a sentence a person reads - and a domain pack only
adds to the neutral base. The schema and the Rust code stay profession-neutral; a profession appears only as a
pack. `knowledge::resolve` decides, `knowledge::gazetteer` finds known names in a text, `knowledge::maintenance`
brings stored phonetic keys and the packs fingerprint up to date when the code that derived them changed.

The extractor reads and forgets: a candidate carries a name, a locator and a count, never a passage. The
resolver never writes. Whatever creates a row does so in the transaction that stores the document's chunks
(`IndexStore::replace_document_with_knowledge`), and a knowledge step that fails never stops the document from
being indexed. See `docs/DECISIONS.md`, "Settled by KB lot 1", "Settled by KB lot 3".

**Ingestion from documents (lot 4).** One Analyse pass, per file, once every embedding exists:

```text
chunks in memory --DeterministicTextExtractor--> candidates --DeterministicResolver--> one KnowledgeDelta
                                                  (names, titles,       (against the store and the entities
                                                   organisations,        this document has just drafted)
                                                   identifiers)
IndexStore::replace_document_with_knowledge( chunks + FTS + document row + delta )   one transaction,
                                             the delta in a savepoint: a failure there loses only the names
after the last file: backfill::refresh_documents  -- stored chunks, no gateway, no original file --
                                             for every document that has no source row, other content, an
                                             older extractor or an older list of names
```

`knowledge::ingest::KnowledgeContext` holds what a pass needs and builds it once (packs and encoder for the
locale, the gazetteer, the key that hashes personal identifiers); `knowledge::backfill` brings documents analysed
earlier up to date; `knowledge::secret` keeps the key. Nothing in this path imports an NLP engine or a gateway
client. `indexing::run` is the pass as it was before; the application calls `indexing::run_with_knowledge`, and
`knowledge_mode = off` writes documents without touching a `kb_*` row.

## Deterministic before generative

For anything the workstation can retrieve or compute locally, deterministic computation comes first.

```text
question
  → resolve any file it names (exact / ambiguous / absent: an ambiguity is answered, never guessed)
  → classify (locale pattern pack: data, never literals in Rust)
      ├─ deterministic path answerable  → compute → answer. No gateway call.
      └─ not answerable                 → retrieval or the aggregate card
                                        → minimum evidence under the cap
                                        → gateway → AIProvider → model
                                        → verify the cited sources; refuse if evidence is insufficient
```

Three consequences that are enforced by tests rather than by intent: a deterministic answer makes zero
HTTP requests; deterministic questions still answer with the gateway stopped; and the same question over
the same file returns the same number under a different model alias. Changing the model must never change
a fact.

The refusal is part of the contract. When the deterministic engine cannot establish an answer it returns
`NOT_DETERMINISTICALLY_ANSWERABLE` with what it does have — the available columns, for instance — rather
than a guess, and when the sources do not carry an answer the product says so rather than generating one.

### The tabular engine's one controlled escalation

The tabular tier's deterministic path (`tabular_answer`) still makes zero gateway calls for any question
its classifier reads. Only when that classifier returns "not recognised", or a residual word looked like
an attempted filter value but matched no real data, does a model get a turn — never to answer, only to
translate the question into a JSON query plan (`tabular::query_plan::QueryPlan`), from the workbook's
**schema** alone: sheet names, row counts, each column's name, type, unit and whether it holds formulas.
No cell value, no distinct value, no row, no file path ever leaves the workstation for this.

```text
question
  → classify deterministically → answerable   → engine::execute → Computed
                                → not answerable (unrecognised, or a filter value matching no data)
      → schema (no values) → model → JSON plan
      → validate, in order:
          1. parse strictly (deny_unknown_fields; an unknown field or op is rejected, not ignored)
          2. sheet reachable under the scope
          3. every named column exists
          4. the operation is allowed on that column's type, and not on a formula column
          5. every eq/in filter value is resolved against the column's real values
             (folded equality; a miss returns close values, never a silent zero)
          6. limits within bounds
      → map to an existing tabular::engine::Operation, run through the unchanged engine
      → InterpretedByModel, or the ordinary nudge when any step above failed
```

Steps 2–4 are not duplicated: the plan is mapped to an ordinary `Operation` and handed to
`tabular::engine::execute`, so a column that does not exist or a formula column is refused by the exact
same code path a deterministically classified question already goes through. Only step 1 (parsing) and
step 5 (filter-value resolution) are this module's own. A gateway that is unreachable, too slow, or
answers invalid JSON degrades to the same nudge an unrecognised question already gave before this
capability existed — the model widens which questions get an answer, and is never load-bearing for
whether any answer is correct.

### The mixed tier: the engines compute, the model only writes

Documents and tables both selected (`GroundingTier::DocumentsAndTables`) is still routed in two steps
(`docs/SESSION-DATA-15-Mixed-Routing.md`, `docs/SESSION-DATA-16-Mixed-Tier.md`). A question that is
clearly and only about the data is answered by the tabular engine alone, before anything else is read —
unchanged since session 15. Everything else reaches `mixed_answer::answer`, a Tauri-free module
`commands::mixed_tier` calls into with an already-classified `tabular_answer::PendingAnswer` and whatever
retrieval already found, for the same `IndexStore`-is-not-`Sync` reason `tabular_answer`'s own
`prepare`/`resolve` split exists: neither the tabular classification nor retrieval's embed-then-search
sequence may hold a reference to the index across a gateway `.await`, so both happen in `commands.rs`,
owning the index, before `mixed_answer::answer` is ever called.

```text
question (documents and tables both selected)
  → tabular classifier, over the tables alone → clearly data-only → tier 2's own engine, unchanged (session 15)
  → otherwise: retrieval over the selected documents
  → tabular_answer::prepare's own PendingAnswer:
      Done(value | structural | a human choice)  → that is the whole answer; no generation, no model
      TryModel (operation recognised, no filter resolved from the question's own words)
          → entity linking: a retrieved excerpt's word anchored against the table's real column
            values (`mixed_answer::entity_link`) — ambiguous between two columns → asked, nothing
            computed; unique → the table's own filter, computed, no model
          → neither: session 14's hidden interpreter, unchanged
  → a computed Value (or no data question at all) and the document excerpts: one turn, two labelled
    blocks ("Document excerpts", "Table results"), MIXED_INSTRUCTION — forbids computing, forbids
    swapping one block for the other
  → the model writes; every number it wrote is checked against the table's own value or an excerpt's
    verbatim text afterward (`mixed_answer::verify_numbers`), and every bracketed citation against what
    was actually supplied (`reject_citations`) — a mismatch is appended, never silently rewritten
  → either side missing (no data question, or no document evidence, or the gateway unreachable for
    generation) degrades to the side that still has something, with no gateway call when that is the
    table alone
```

The model never receives a cell value or a row here either: the table block is built from
`tabular::escalation::format_evidence`, the exact function tier 2 keeps unsent for this session, which
already refuses to turn a row list into evidence. A mixed answer's `Source`-shaped half is `Computed`
(the table) beside `Extracted`/`Recognised` (the documents) — never `ModelAsserted`: the model's prose is
checked against both, not trusted as a third kind of fact.

## Runtimes are adapters, never the home of business logic

Tooling (Open WebUI Tools, MCP, Hermes, OpenClaw, whatever comes next) will change faster than the need. We
anticipate by owning the contracts, not by betting on a harness.

| Family | Examples | Value | Risk | Posture |
| --- | --- | --- | --- | --- |
| OpenAI-compatible API | Ollama, llama.cpp, vLLM | Inference portability | Dialect differences on tools and vision | **Primary port from v1** |
| MCP | File and IDE servers | Tool standard | Young spec, uneven servers | **Tool port** once the catalogue exists |
| Self-hosted UI | Open WebUI | Admin, demos | Persistence by default, UI lock-in | UI adapter, **not** the product |
| All-in-one agents | Hermes, OpenClaw | Memory, skills, fast demos | Broad surface, sometimes too many disk rights | Optional proof of concept **behind** our tools |
| Editor plugins | Continue, Cline, Aider | Technical profiles | Third-party telemetry, wide scope | External clients, `chat` scope only |

Design rules that follow:

- **Hexagonal.** The domain (plan, approval, audit, policy) imports no vendor SDK.
- **Two stable ports.** LLM is OpenAI-compatible; tools are our schemas, and MCP is only a façade over them.
- **A plan is our own JSON.** If a runtime disappears, the manifest still applies.
- Runtime feature flags, so a harness can be tried without exposing it to the pilot.
- A runtime that cannot honour no-store is out of production.
- Before adding an "agent" dependency, ask whether it is an adapter or whether the business logic will end
  up living inside it. If the second, refuse.

Scenarios this survives: Ollama changes licence or API (point the gateway at llama.cpp or vLLM, same
aliases); Open WebUI breaks its Python tools (the client and MCP remain); MCP becomes the single standard
(expose the same catalogue, rewrite nothing); a much better model appears (change the `cabinet-chat` alias,
not the clients).

## Bridges to the usual software

Every client targets the same OpenAI-compatible base URL with a per-person key and an **alias** as the model
name, never a raw weight name.

```json
{ "provider": "openai", "model": "cabinet-chat",
  "apiBase": "https://ia.cabinet.local/v1", "apiKey": "sk-user" }
```

| Wave | Surface | Role |
| --- | --- | --- |
| v1 | Gateway plus the native window | Single contract, no browser for the practice |
| v1 | Open WebUI through the gateway | Admin and test bench, no sensitive knowledge |
| v2 | Explorer context menu, Word add-in | "Send to the assistant" without leaving the file |
| v2 | Connection sheet for Continue / Cline / Aider | Technical profiles |
| later | Outlook and document-management connectors | A connector is an adapter, not a new backend |
| reserved | `/v1/audio/*` and `/v1/jobs` | Local voice and LAN mobile |

Specific risks when connecting an IDE plugin: many extensions phone home, they send the content of open
files, and some resend the whole history every turn. Therefore: an allow-list of permitted tools, `chat`
scope only, never `apply` or `move` exposed to plugins, and context caps on the gateway.

Business code never addresses `http://ollama:11434`. It addresses the gateway.

## Isolation

There is no server-side index to partition, because the server does not hold the files. Partitioning is
Windows accounts plus local collections plus API keys. A network share is indexed **on the workstation that
already has access to it**.

## Work folder

Disk writes happen **only** inside a chosen work folder (allow-list), like a project root. Not the whole
Windows profile, not the whole Documents folder, and never Open WebUI Computer. Excerpts go to the server,
the plan is approved, deletions go to a dedicated trash folder. See `docs/CLIENT.md`.

## Portability

`LLM_BASE_URL`, aliases and paths are configuration. Development uses the PC with a small model; production
is the Mac mini's URL with more capable models. Sizing: `docs/HARDWARE.md`.

**Windows and macOS are both mandatory targets for the client**, decided 19 September 2026. That outranks
convenience in every technology choice: a library, runtime or engine that exists on only one of them is
disqualified however good it is on that one. It is what rejected the platform OCR engines, which would
have been two implementations with two accuracies and two failure modes rather than one capability.

Three rules keep it true rather than hoped for. Directories come from `app.path()`, never from a literal.
A platform difference lives in a paired `#[cfg]` block in the module that owns the platform concept — the
work-folder policy, the bundler configuration — and never inside a capability such as extraction, OCR or
retrieval. And a test must assert the same rule on both systems: a test whose fixture is a Windows path
literal proves nothing on macOS, where a backslash is an ordinary filename character. Current state,
including the gaps: `docs/SPRINT-2.5-ASSESSMENT.md` section O.
