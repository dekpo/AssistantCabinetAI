# Sprint 2 architectural assessment

**Written 18 September 2026. Reviewed and accepted the same day: the four decisions at the end were all
confirmed as recommended, and are now recorded in `docs/DECISIONS.md`. No code has changed yet.**

Scope: the revised Sprint 2 deliverable — local document **and** tabular retrieval on the workstation,
deterministic factual spreadsheet lookup, provenance, and LLM escalation only when required — assessed
against what `AssistantCabinetAI` already is, and against the proven implementation in `LocalGridMind`
(`develop`, 38 commits, inspected locally at `C:\Users\elise\Documents\CURSOR\LocalGridMind`).

Read with `docs/ROADMAP.md` (dates), `docs/ARCHITECTURE.md` (contracts) and `docs/RETRIEVAL.md` (the
pipeline this sprint implements).

---

## A. Existing components to keep unchanged

Sprint 1 landed a clean hexagonal slice. Nothing in it needs rewriting for Sprint 2.

| Component | Why it stands |
| --- | --- |
| `providers/base.py` — `AIProvider` | Already the right port: `generate`, `embed`, `rerank`, `check_health`. `embed` exists and is unused, which is exactly the shape Sprint 2 needs |
| `providers/ollama.py` | The only implementation, and it stays the only one. No multi-provider routing in this sprint |
| `core/no_store.py` | `MetadataOnlyFilter` drops any record not built by `log_metadata`. This is the isolation guarantee, in code rather than in review. Extend the tests, not the mechanism |
| `core/register.py` | `extra="forbid"` plus hashes. A later commit cannot slip a spreadsheet row in without Pydantic refusing |
| `core/prompts.py`, `core/locales.py`, `prompts/` | English body plus a rendered output-language directive. Tabular evidence changes the body's *content*, not this mechanism |
| `src-tauri/work_folder.rs` | The allow-list is done, tested (11 `cargo test`), and platform-derived. File discovery in Sprint 2 sits **inside** it and adds no new policy |
| `src-tauri/error.rs` | Machine code plus structured data. Every new failure becomes a variant here, never a sentence |
| `src/guards/sources.test.ts` | The language contract as a test. It will catch the mistake Sprint 2 is most likely to make — see section C |
| `compose.yaml`, settings on both sides | Server URL, alias, work folder, locale are already configuration |

## B. Existing components requiring small refactoring

These are additive changes to existing files, not rewrites. Roughly two days of work in total.

1. **`apps/server`: add `POST /v1/embeddings`.** `AIProvider.embed` and `OllamaProvider.embed` exist;
   there is no route. Same shape as `chat.py`: resolve the alias, cap the input, record metadata only.
   Known gap, already logged in `docs/ROADMAP.md` as inherited from Sprint 1.
2. **`api/chat.py`: the context cap becomes evidence-aware.** `enforce_context_cap` counts characters
   across all messages. That is still correct, but the register should also carry `evidence_count` and
   `evidence_kind` (`document` / `tabular`) so an isolation test can assert *how much* crossed the
   boundary, not only that nothing was stored.
3. **`src-tauri/commands.rs`: `AppState` gains an index handle.** Today it holds settings plus a gateway
   client. It needs an index and an inventory store, both behind traits, opened per work folder.
4. **`src-tauri/settings.rs`: two new settings.** Index location (defaults inside the app-data directory)
   and the context cap in characters. No constants.
5. **`fixtures/gp-sandbox/`: extend.** It holds only `.txt`. Sprint 2 needs native-text PDFs, one DOCX,
   one scanned-image PDF reserved for the OCR contract, and — new — one `.csv` and one `.xlsx`. The
   tabular fixtures must be a plausible **practice schedule**, not a finance model, or the engine will be
   tuned for the wrong shape.
6. **`Cargo.toml`: new dependencies.** Candidates, each needing a licence line the way `models/` already
   does: `calamine` (xlsx/xlsm/xls, pure Rust), `csv` + `encoding_rs`, `rusqlite` (`bundled`, FTS5),
   `quick-xml` for DOCX, a PDF text extractor, `chrono`.

## C. New Sprint 2 interfaces

All of this lives in `apps/desktop/src-tauri`, on the workstation. **None of it goes in `apps/server`.**
The server must never receive a document, a workbook, or a row it did not strictly need — so the engine
that reads them cannot live there.

That single constraint decides the biggest question in this assessment: **the deterministic tabular
engine is written in Rust, and `LocalGridMind` is a specification rather than a dependency.** Its engine
is Python and pandas. Shipping a Python sidecar inside a Windows installer four weeks before deployment
on a 2019 practice PC trades a certain schedule risk for an uncertain saving. Rejected. What we reuse is
the design, the thresholds, the refusal behaviour and the test corpus — which is the expensive part.

### Ports (Rust traits, no `tauri::` imports in any of them)

```text
DocumentSource      discover(work_folder) -> Vec<DiscoveredFile>        # generic, extension-driven
TextExtractor       extract(path) -> ExtractedDocument                  # pdf / docx / txt / md
Chunker             chunk(ExtractedDocument) -> Vec<Chunk>              # keeps file, page, section
Embedder            embed(Vec<String>) -> Vec<Vector>                   # GatewayEmbedder today, ONNX later
IndexStore          upsert / search_lexical / search_vector / clear     # SqliteIndexStore today
RetrievalService    search(query, scope) -> Vec<Evidence>

TabularDataSource   open(path) -> Workbook                              # CsvSource, XlsxSource
TabularInventory    build(Workbook) -> WorkbookInventory                # structure and facts
InventoryStore      put / get_by_hash / invalidate                      # SqliteInventoryStore today
TabularAnalysis     analyze(query, scope) -> TabularOutcome
```

### The common source model

Documents and workbooks are different internally and identical at the boundary. One type, three parts:

```text
Source {
  origin:     { relative_path, sha256, modified_at }
  locator:    Document { page, section, chunk_id, char_range }
       |      Tabular  { sheet, header_row, column, row_range }
  derivation: Extracted                                   // copied from the file
       |      Computed { operation, operands, row_count }  // we calculated it
       |      FormulaStored { expression }                 // the file says so; we did not verify
       |      ModelAsserted                                // the LLM said it
}
```

`derivation` is the part worth arguing for. `LocalGridMind` keeps the verified/unverified distinction in
its prose — "not a total", "SUGGESTED FORMULA", "the stored formula is X, not Y". Prose is not a
boundary. Making it a field means the UI can render a computed total differently from a model sentence,
and a test can assert that no answer labelled `Computed` ever passed through the LLM.

### Tauri commands (the only bridge, as in Sprint 1)

`index_work_folder`, `index_status`, `ask` (documents), `ask_tabular`, `workbook_inventory`. Each takes
and returns plain serde structs — no handles, no absolute paths outside the work folder — so the same
contract can later be served over HTTP without redesigning it. See section I.

### The language trap, and the one rule that avoids it

`LocalGridMind` fuses three things in `src/core/lookup.py`: English regexes that classify the question,
the operation, and the English sentence that answers it. Copying that shape into `src-tauri` would put
French regexes and French sentences in Rust, and `src/guards/sources.test.ts` would fail the build — it
rejects non-ASCII characters and French markers in every `.rs` file. That test is right and the design
would be wrong.

So the three parts separate:

| Part | Where it lives | Why |
| --- | --- | --- |
| Question patterns | A locale-keyed pattern pack, loaded as a resource (`fr-FR`, `en-US`) | Data, not code. A new language is a new file |
| The operations | Rust, language-neutral: count, distinct, sum, min, max, group-sum, filter, max-row | Changing the locale must not change a number |
| The sentence | React catalogues, from a structured `TabularAnswer` | Same contract as every error code today |

This is strictly better than the source we are borrowing from, and it costs nothing extra.

## D. LocalGridMind functionality worth reusing

Ranked by how much it would cost to rediscover. Everything here is a **principle plus its thresholds**,
reimplemented in Rust, not a file copy.

| What | Where in LocalGridMind | Why it matters here |
| --- | --- | --- |
| Inventory built once at attach, answered from cache, file never re-read | `core/packs.py`, `core/lookup.py:166 answer_from_json` | This is the whole architecture. It is why answers are instant and why the model is optional |
| Aggregates computed over **every** row, never a sample | `core/stats.py:152` — "Do not pass a `head()` sample" | The failure it prevents is silent and severe: a group outside the first 2,000 rows has the largest total |
| Group **sum** and largest **single row** are different facts, labelled separately | `core/stats.py:567 _group_sums`, `:626 _max_rows` | The subtlest correctness point in the whole engine. Both can name the same group; often they do not |
| A formula cell contributes **no value** to the facts | `core/stats.py:406 _cell_value`, `:419 _is_formula_value` | A cached formula result must never be presented as a verified fact. Directly answers requirement §9 |
| Refuse to build a facts card for a formula-heavy file or a non-grid sheet | `core/stats.py:131 file_allows_tabular_stats` (≤20 formulas), `:136 sheet_looks_tabular` (≥8 rows, ≥2 columns, formula ratio ≤5%) | Knowing *when not to answer* is the feature. Start with these numbers; they are empirical, not theoretical |
| Column matching fails closed on ambiguity | `core/stats.py:354 match_named_column` — returns `None` when two columns tie | The alternative is confidently answering about the wrong column |
| Suppress identifier columns from numeric stats; require a numeric share ≥0.5 | `core/stats.py:488 _numeric_stats` | Nobody wants the sum of a patient-reference column |
| The closed answer carries the available columns | `core/lookup.py:97 _CLOSED`, `:478 _available_columns_line` | This is `NOT_DETERMINISTICALLY_ANSWERABLE` done usefully: the refusal tells you what *would* work |
| Intent classified before execution, in a deliberate order | `core/lookup.py:104 classify_inventory_intent` | Ordering matters: a cell-reference question must not be captured by "which is largest" |
| The model receives a compact aggregate card, never rows | `core/stats.py:286 prompt_stat_lines` | Our minimum-evidence rule, already implemented |
| The model's claims are checked against the inventory afterwards, with a correction appended | `core/ground.py:30 ground_model_reply` | The reasoning path's safety net. The draft stays visible; the verified fact wins |
| The inventory data model | `core/inventory.py:101-177` — `ColumnInfo`, `SheetInfo`, `FileInventory`, `PackInventory` | Field-for-field close to what requirement §7 asks for. Port the shape |
| The test corpus | `tests/test_stats.py` (257 lines), `tests/test_lookup.py` (313), `tests/workbook_fixtures.py` (183) | The most reusable asset in the repository. Translate these cases into `cargo test` and the Rust engine inherits the bugs already fixed |

## E. LocalGridMind functionality that should NOT be copied

| What | Why not |
| --- | --- |
| Streamlit, `st.session_state`, `ui/chat_panel.py`, `ui/library.py` | A different product's UI. We have a Tauri window and our own settings and history design |
| English prose assembled inside the engine (`core/lookup.py` returns sentences) | Breaks our language contract. The engine returns a structured answer; the catalogue writes the sentence |
| Finance vocabulary: `_AMOUNT_HINTS`, `_GROUP_HINTS`, `_WACC_TERMS`, the cost-of-capital ranking buckets in `_where_sort_key` | Requirement §3 is explicit that this must be profession-independent. Keep the *idea* of ranking hints; make the hint list locale and profession data |
| The formula index, named ranges, external workbook links, `ground.py`'s A1-reference resolver | A financial-model feature. Sprint 2 needs formula **awareness** (detect, disclose, refuse), not a formula browser. Requirement §23 says no Excel engine |
| `.xls` via Excel "Save As" COM automation (`core/legacy_xls.py`) | Requires Excel installed, Windows-only, fragile. `calamine` reads `.xls` values directly; that is enough |
| Copying uploads into an app-owned store (`core/packs.py`, `data/uploads/<uuid>/`) | We have a work folder. Requirement §7 says do not store unnecessary copies. Inventory by path plus hash; copy nothing |
| pandas | Not available in the Rust core, and not needed. The operations we require are a few hundred lines |
| `llm/runtime.py` (llama-cpp in-process) | We have a gateway and `AIProvider`. Business code never loads a model |
| Attaching files "per chat" | Our scope is the work folder, which is a persistent allow-list, not a per-conversation upload |

## F. Document pipeline

```text
work folder
  → discovery (extension allow-list: .pdf .docx .txt .md .csv .xlsx; symlinks refused; inside the allow-list only)
  → TextExtractor per type
  → normalisation (whitespace, encoding), metadata (path, sha256, modified_at, page count)
  → Chunker, preserving file / page / section
  → Embedder (gateway today)
  → IndexStore (SQLite: FTS5 lexical + stored vectors, brute-force cosine)
  → RetrievalService.search → Evidence[] with Source.derivation = Extracted
  → context selection under a character cap
  → gateway → LLM → answer citing file, page, passage
  → insufficient evidence → refusal, no generation
```

Unchanged from `docs/RETRIEVAL.md`. This is the milestone A chain and it already has a written design.
Extraction failure is reported, never guessed around: an empty extraction blocks classification.

## G. CSV/XLSX pipeline

```text
work folder
  → discovery (same mechanism, same allow-list — no separate tabular loader)
  → TabularDataSource::open       CSV: encoding + delimiter detection; XLSX: calamine
  → parse: header row, dimensions, per-column inferred type, formula presence
  → TabularInventory::build       full pass over every row, not a sample
       - column inventory: name, type, null ratio, distinct count, samples
       - facts, only if the gate passes: distinct values (≤64), min/max/sum per numeric column,
         date/year ranges, top-5 group sums, largest single row
       - gate: file not formula-heavy AND sheet looks like a data grid
  → InventoryStore, keyed by (relative_path, sha256) — the file changes, the inventory invalidates
  → TabularAnalysis::analyze(query)
       → Answered { value, Source{ Tabular locator, derivation: Computed{operation, operands, rows} } }
       → NotDeterministicallyAnswerable { reason, available_columns }
```

Nothing is flattened into text chunks. A workbook is never embedded.

**The French adaptation that has no equivalent upstream.** `LocalGridMind` is an English, US-format
product. French practice exports are semicolon-separated, cp1252 or UTF-8-BOM, decimal comma,
`dd/mm/yyyy`. Delimiter and encoding detection, and locale-aware number and date parsing, are new work —
a day, but a day that decides whether any of the rest is correct.

**The real gap in the borrowed design.** `LocalGridMind` extracts *year ranges* only
(`core/stats.py:423 _year_ranges`), not dates. Requirement §8 asks for date filtering, and the motivating
example — "how many appointments on Monday" — needs real date handling: parse to a date, expose day of
week, filter on a range. This is the one place where we must build past the reference implementation.

## H. Deterministic lookup → AI escalation boundary

```text
question
  → classify against the locale pattern pack
      │
      ├─ tabular intent recognised, scope has an inventory with facts
      │     → TabularAnalysis::analyze
      │         ├─ Answered            → render from the catalogue. NO gateway call. Ever.
      │         └─ NotDeterministically→ fall through, carrying the reason
      │
      └─ otherwise
            → RetrievalService.search (+ the aggregate card when the scope is tabular)
            → evidence under the cap
            → gateway → AIProvider → LLM
            → verify the answer's citations against the evidence set; drop or correct what does not resolve
            → still insufficient → refusal
```

Three properties that must be tests, not intentions:

- A deterministic answer performs **zero** HTTP requests. Assert it with a provider double that panics
  if called. Requirement §24 asks for exactly this.
- With the gateway stopped, every deterministic question still answers. This is the architectural test in
  requirement §25 and the one that actually proves the LLM is not the source of truth.
- The same question against the same file returns the same number regardless of the configured model
  alias.

The escalation must carry *why* it escalated. "Not on the card, available columns are X, Y, Z" is useful
to the user even when the model then answers; silently escalating hides the fact that no number was
verified.

## I. Mobile-ready boundaries

Nothing mobile is implemented. Three disciplines keep the door open at zero cost, and they are worth
following because they also make the code testable today:

1. **No `tauri::` import in any engine module.** Extraction, index, inventory, analysis are plain Rust
   taking paths and returning data. Tauri commands are a thin adapter over them. This is the only one of
   the three that would be expensive to retrofit.
2. **Commands are request/response data.** Every new command takes one serde struct and returns one. No
   channels except the existing chat stream, no handles, no absolute paths leaking to the webview. A
   workstation-local HTTP listener could serve the same structs later; we do not write it.
3. **Provenance travels with the answer, not alongside it.** A `Source` is self-describing — path, hash,
   locator, derivation. A future client that never saw the file can still render the citation.

What a mobile client would later need and must not be designed away now: device identity, per-device
scopes, and the rule that the phone talks to the workstation or the gateway, never to Ollama. Those are
V1 concerns. `docs/DECISIONS.md` already reserves `/v1/jobs` and `/v1/audio/*` for them.

## J. Risks of over-engineering

**The schedule is the risk.** Today is 18 September. `docs/ROADMAP.md` gives Sprint 2 the window
24–30 September and hangs milestone A on it. The revised deliverable adds thirty numbered items,
including a tabular engine, to a one-week sprint that did not previously contain one. Delivering both
pipelines by 30 September is not achievable, and attempting it puts the document chain — the thing
milestone A is actually judged on — at risk.

**The pilot has no spreadsheets.** `docs/PILOT-GP.md` is unambiguous: about 20 specialist and imaging
reports a day, 10 to 20 lab results, mostly PDF; accounting left the scope in September 2026 when the
accountant's software took it over. There is no CSV and no XLSX anywhere in her measured workflow. The
tabular layer earns nothing for milestone B on 14 October. It is platform work for the *next* profession
— the lawyer case lists and accountant exports in `docs/DISCOVERY.md` — and it should be scheduled and
justified as such, honestly, rather than smuggled into the pilot sprint.

That argues for sequencing, not for dropping it. The recommendation in section K is to split Sprint 2,
keep milestone A on documents, and give the tabular engine its own window after it — building it against
the boundaries defined here so nothing is rewritten later.

Smaller traps, each cheap to avoid now:

| Trap | Avoidance |
| --- | --- |
| Building a generic query planner for spreadsheets | Nine named operations. A tenth is a decision, not a configuration |
| An inventory schema that tries to describe every possible workbook | Describe what the nine operations need. Extend when an operation needs more |
| Abstracting `AIProvider` further for future runtimes | The port is already right. Requirement §13 says no multi-provider routing |
| A second database "for vectors" | SQLite with brute-force cosine. A work folder holds tens of files |
| Formula evaluation creeping in | Detect, disclose, refuse. The gate exists precisely so we never need to evaluate |
| A profession-specific column heuristic | Hints are locale data. A medical schedule and a legal case list must use the same engine |

## K. Exact implementation sequence

Each step ends green and is independently useful. One branch per step, per `.cursor/rules/git-workflow.mdc`.

**Documentation, before any code.** Reframe `docs/ROADMAP.md` on the V0→V3 progression, create
`docs/PLATFORM-VISION.md` for the long-term direction, and add the new ports plus the `Source` model to
`docs/ARCHITECTURE.md`. Record the Rust-not-Python decision and the Sprint 2 split in `docs/DECISIONS.md`.
`docs/VISION.md` already carries most of the platform framing and should be folded in rather than
duplicated.

**Sprint 2a — documents (24–30 September, carries milestone A).**

1. `POST /v1/embeddings` on the gateway, with an isolation test. Unblocks everything downstream.
2. Extend `fixtures/gp-sandbox/`: native-text PDFs, one DOCX, one scanned PDF for the OCR contract.
3. File discovery over the work folder, extension allow-list, symlinks refused.
4. Extractors: TXT and MD, then PDF, then DOCX. Empty extraction is a reported failure.
5. Chunking that preserves file, page and section. The `Source` type lands here.
6. `IndexStore` trait plus `SqliteIndexStore` — FTS5 and stored vectors.
7. `Embedder` trait plus `GatewayEmbedder`.
8. `RetrievalService.search`, context selection under the cap.
9. Citation rendering in React; refusal when evidence is insufficient.
10. Tests: retrieval, attribution, refusal, and the isolation test proving the server holds nothing.

**Milestone A, 30 September:** three sourced answers, one refusal, isolation passing, on fictional files.

**Sprint 2b — tabular (after milestone A, sequenced against Sprints 3 and 4 at review time).**

11. `TabularDataSource` plus `CsvSource`: delimiter and encoding detection, French formats, full parse.
12. `WorkbookInventory` and the column inventory. `InventoryStore` keyed by path and hash.
13. The facts pass over every row: distincts, numerics, date and year ranges, group sums, max row.
14. The nine operations plus `NotDeterministicallyAnswerable` carrying available columns.
15. The locale pattern pack and the classifier; `TabularAnswer` rendered by the React catalogues.
16. The test that matters: a provider double that panics on call, exercised by every deterministic case.
17. `XlsxSource` via `calamine`, with the formula-heavy and data-grid gates. Formula workbooks are
    inventoried and refused for facts, with a reason the user can act on.
18. Escalation: the aggregate card as structured evidence into the existing chat path, plus verification
    of the model's citations against the evidence set.
19. Port the `LocalGridMind` test corpus into `cargo test`.
20. Regression: the full Sprint 1 and 2a suites still green; the gateway-stopped test passes.

---

## Decisions taken

All four confirmed on 18 September 2026 and recorded in `docs/DECISIONS.md`.

1. The deterministic tabular engine is **Rust on the workstation**, and `LocalGridMind` is a
   specification and test corpus rather than a dependency. No Python sidecar in the installer.
2. **Sprint 2 splits.** Documents carry milestone A on 30 September. Tabular becomes sprint 2b, dated
   after milestone B, on the grounds that the pilot's workflow contains no spreadsheets. Placing it
   earlier would displace either her three workflows or the deployment.
3. **XLSX scope for the first implementation:** data-grid sheets only. Formula workbooks are inventoried,
   disclosed, and refused for factual answers. No formula evaluation.
4. **Question classification is locale data, not Rust code**, and answers are rendered by the React
   catalogues from a structured result.
