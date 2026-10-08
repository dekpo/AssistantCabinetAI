# Lot 0 - Measure and unfreeze

8 October 2026 · branch `feat/kb-measure-and-unfreeze` · based on `kb/integration` at `0aa85df` (the programme set-up commit; baseline tag `kb-baseline` = `f27505f`) · tab "KB 0 - Measure and unfreeze"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The repository's own rules now allow the Knowledge Base work (decisions, roadmap, always-applied rule), every question and every Analyse pass is timed as numbers and machine codes only, and the lexical half of retrieval is held to the user's selection. Nothing is visible on screen. The measurement the programme is judged against (`00-baseline.md`) is a template for the owner to fill in, with a generator for the fictional corpus it needs; the one thing it must tell her, which stage dominates the 30-file case, is **not known yet** because it needs the real stack.

## What changed

**Governance (documentation and one always-applied rule)**

- `docs/DECISIONS.md` - new sections "Knowledge Base and the product direction (8 October 2026)" (D1-D10, the withdrawn 14 October milestone, what moves from frozen to planned) and "Settled by KB lot 0".
- `docs/ROADMAP.md` - the two dated milestones replaced by capability milestones K-A to K-E and a parallel-tracks table; milestone A kept as history, milestone B struck through as withdrawn; sprint 4 dates withdrawn; "After 14 October" retitled "Later, with no date".
- `.cursor/rules/v0-sprint.mdc` - milestone paragraph rewritten, "Frozen until 14 October" now "Frozen" without a date, voice/organisation/calendar/connectors moved to planned. Every boundary that was not being changed is untouched.
- `docs/RETRIEVAL.md`, `docs/OPERATIONS.md` - the scoped lexical search; the two optional log files.

**Measurement (Rust)**

- `src-tauri/src/knowledge/mod.rs`, `knowledge/diagnostics.rs` - new. `StageTimer`, `timed_ms`, `RetrievalDiagnostics` (+ `RetrievalPath`, `RetrievalPlan`), `AnalysisTimings` (+ `AnalysisPass`, `FileTiming`), the workbook-load recorder, `append_timing_line`.
- `src-tauri/src/lib.rs` - `pub mod knowledge;`.
- `src-tauri/src/settings.rs` - `show_diagnostics`, `write_timing_log` (both default false, `serde(default)`).
- `src-tauri/src/commands.rs` - `ask_with_sources` wraps the answer, times it and logs it; every tier (`sourced_answer`, `tabular_tier`, `mixed_data_only_tier`, `fill_tier`, `mixed_tier`) fills the stages; `Writer::write` returns the first-token and total generation time; `AskAnswer.diagnostics`; `index_work_folder` and `index_data_folder` write `analysis-timings.jsonl`.
- `src-tauri/src/indexing.rs` - per-file and aggregated hash/extract/OCR/chunk/embed/write times, batches, characters, repeated-text count, slowest five by position; `IndexSummary.timings`; a `TimedOcr` decorator around the `OcrProvider` port.
- `src-tauri/src/data_folder.rs` - the same for the Data pass (`build_inventory_ms`, `write_ms`).
- `src-tauri/src/tabular_answer.rs` - `load_workbook_cached` reports each load and whether the cache served it (5 lines).
- `src/lib/ipc.ts` - `AppSettings`, `IndexSummary`, `AskAnswer` gain the matching fields and types. Diagnostics and timings are rendered nowhere.
- `src/components/SettingsDialog.tsx`, `src/locales/{fr-FR,en-US}.json` - added after the first hand-over, at the owner's request: an "Avancé" / "Advanced" group with the two checkboxes (`writeTimingLog`, `showDiagnostics`), catalogue parity kept.

**Fixes that need no KB**

- `src-tauri/src/index_store.rs` - `IndexStore::search_lexical_in(query, limit, relative_paths)`.
- `src-tauri/src/retrieval.rs` - `File`, `Files` and `CurrentFolder` use it, `WholeFolder` keeps `search_lexical`; `search_per_document` runs the query once for its whole list (`search_scoped_with_hits`); `SearchOutcome` and the `*_counted` variants return `chunks_considered`. `search_scoped` and `search_per_document` keep their signatures.

**Baseline, human test, tooling**

- `docs/test-reports/knowledge-base-pass-1/00-baseline.md` - the template, and the numbers the agent could measure.
- `docs/test-reports/knowledge-base-pass-1/human-tests/lot-00-measure-and-unfreeze.md` - the human test.
- `fixtures/kb-baseline/make_corpus.py`, `README.md` - deterministic generator of 10/30/50 fictional letters and three tables.
- `src-tauri/tests/analysis_timings.rs` (5 tests) and `tests/embedding_parallelism_bench.rs` (`#[ignore]`).

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| Timings are always computed and returned; the **log** is the opt-in | A few clock reads cost nothing, and a later lot's developer panel needs the values without a second switch | `DECISIONS.md` "Settled by KB lot 0" |
| A refused question (`InsufficientEvidence`, `ScopeUnavailable`) is logged; a stopped one (`ChatCancelled`) is not | A refusal is a real timing (embedding and search were spent); a stop measures the user's patience | `commands::ask_with_sources` |
| Workbook loads reach the diagnostics through a thread-local read straight after the synchronous stretch that filled it | The loader is five calls below the code that owns the diagnostics and all of them are synchronous; threading a parameter through `tabular_answer` (3 500 lines) was a larger and riskier change | `knowledge::diagnostics::record_workbook_load` doc |
| The slowest five files are identified by **position in the pass** | Master section 0.2; a log must not become a list of file names | `FileTiming` |
| `AnalysisTimings` also counts chunks whose text is byte-identical to an earlier chunk of the same pass | Section 0.4 asks how many there are; it is the only way to answer without reading documents. Only a 64-bit hash is held, for the length of the pass, never serialised | `AnalysisTimings::note_chunk_text` |
| `search_per_document` restricts its single lexical query to the listed files | Section 0.3 says "behaviour and output identical" for a fixture folder, which holds; for a list that is a subset of the index it also removes the crowding the other fix is about | `retrieval.rs` |
| The typed-cache pre-warm (0.4) is **not built** | Cold first question measured at 0.5 to 0.8 s for 100 000-row CSV and 50 000-row XLSX on the development machine, once per workbook; pre-warming doubles the data written per pass and reopens the residual-risk question. To be re-decided with the owner's `workbookLoadMs` | `00-baseline.md` section 9.1 |
| `kb-programme.mdc` and `project-context.mdc` are left as they are | Lot 0 names three files to rewrite; `project-context.mdc` repeats no milestone, and `kb-programme.mdc` is the programme's own rule (its "until it is merged" sentence is true until then and harmless after) | this report |

## Deviations

- **A guard the master does not mention.** `src/guards/sources.test.ts` (run by `pnpm run test`, not by `cargo test`) reads every `.rs` file under `src-tauri/src` and fails on any non-ASCII character and on any line containing a French marker word (`le la les des une est erreur fichier dossier envoi veuillez aucune`), **test fixtures included**. My first fixtures failed it (three files). They are now English or marker-free. `tests/` is not scanned. This will bite every later lot that writes a French fixture inside `src/`; recorded in the status file.
- Master section 4 says `search_per_document` "takes files in path order". It takes them in the order the caller gives (the inventory's deterministic order). Unchanged either way.
- Section 0.2 asked for `workbook_load_ms (and workbook_cache_hit: bool)`; they are per question and aggregated over the workbooks the question read, `workbookCacheHit` meaning "every workbook came from the cache" and false when none was read.
- No deviation from the code facts of the master: `fts_match_expression`'s four-character rule, the six lexical places, the unrestricted `search_lexical`, `IndexStore` without pragmas, `ask_with_sources` shape - all verified in the code before changing it.

## Invariants

Lot 0 adds no KB row, so most of I1-I14 are not yet in play. Touched:

| Invariant | Proof |
| --- | --- |
| I8 - nothing the KB stores or computes is logged as text; diagnostics hold ids, counts and milliseconds | `knowledge::diagnostics::tests::a_timing_line_built_from_a_populated_store_holds_only_numbers_and_codes` (plants names, a phone number and paths in a populated store, searches, builds the line, asserts none appears and every string value is a known code); `tests/analysis_timings.rs::the_timings_of_a_pass_carry_no_file_name_and_no_text` (a real pass; every field is a number, a list or the `pass` code) |
| I9 - no new model-facing string | None added. The existing `..._stays_neutral_about_who_the_user_is` guards are untouched and green |
| I10 - machine codes only in Rust | `RetrievalPath`, `RetrievalPlan`, `AnalysisPass` serialise as `snake_case` codes; the language guard above passes |
| I13 - no `cfg` in the KB, no platform path literal in a test | No `cfg` anywhere in the lot; fixtures use `/` and temp directories |

## Verification

All from the repository, Windows 11, debug profile unless stated. Baseline measured on `0aa85df` before the first change.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` - `--lib` | 500 passed, 2 ignored | **523 passed**, 2 ignored |
| `cargo test` - the integration files (17 before, 19 after) | 142 passed, 1 ignored | **147 passed**, 2 ignored (`analysis_timings` +5; `embedding_parallelism_bench` is the new ignored one) |
| `cargo clippy` | 20 warnings in the lib | the same 20, identical list |
| `rustfmt --edition 2021 --check` on each touched file | `index_store.rs` 4 diffs, `commands.rs` 11, `tabular_answer.rs` 43, the rest 0 | the same 4, 11 and 43 (all pre-existing); every other touched file and every new file clean. No plain `cargo fmt` was run |
| `pnpm run build` (`tsc --noEmit` + vite) | OK | OK |
| `pnpm run test` | 331 passed, 26 files | **333 passed**, 26 files (+2: the language guard on the two new Rust files; unchanged by the settings checkboxes, re-run after adding them) |
| `apps/server` `pytest` | not run | not run (nothing there changed) |

Not run: `embedding_parallelism_bench` (needs the real gateway; none was running on this machine) and the application window.

## Measured

Development machine: Intel Core 7 150U, 12 threads, 23.6 GB RAM, Windows 11, release build, no gateway. Method: `cargo test --release --test tabular_column_cache_bench -- --ignored --nocapture`.

| Workbook | Rows | Size | First analysis | One question with a cold typed cache | Ten questions, average |
| --- | --- | --- | --- | --- | --- |
| CSV | 100 000 | 5.6 MB | 415 ms | 526 ms | 530 ms |
| XLSX | 50 000 | 17.0 MB | 438 ms | 807 ms | 809 ms |

(The 1 October session recorded 819 ms and 1.52 s for the same files on the same kind of machine; this one is faster. Same bench, not the same hour of the day.)

**Which stage dominates the 30-file case: not measured.** It needs the model, the embedding model and the owner's machines. The protocol is `00-baseline.md`; the answer is section 8 of that file and goes into the report of the next lot.

## Not done

- The owner's measurement (10/30/50 documents, one mixed run): a protocol and a generator are delivered, the numbers are hers. Until she has run it, no claim about where the seconds go.
- Section 0.4 items: neither is built. The pre-warm is decided against for now with the numbers above; the `chunk_text_sha256 -> vector` cache is proposed in the baseline file only if the repeated share is material; the two-in-flight embedding measurement is written (`embedding_parallelism_bench`) but not run.
- Rendering diagnostics anywhere, as specified.
- Nothing was run on macOS. No code in the lot is platform-specific.

## Documentation

`docs/DECISIONS.md`, `docs/ROADMAP.md`, `.cursor/rules/v0-sprint.mdc`, `docs/RETRIEVAL.md`, `docs/OPERATIONS.md`, this report, `00-baseline.md`, the human test, `fixtures/kb-baseline/README.md`. `AGENTS.md` is **not** edited; its proposed patch is below.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-00-measure-and-unfreeze.md` - **proposed, not run** (step B2 now uses the new checkbox). Part A: the existing product on the first human pass's fixtures (non-regression). Part B: the log is absent by default, then holds numbers and codes only once switched on. Part C: the baseline measurement. The agent drove neither the window nor a gateway; every observation in that file is the owner's to make.

## Open questions

1. (Answered 8 October 2026: yes, switches added now; the gateway of the GPU machine on the network is a second measurement target, see `00-baseline.md`.) `.cursor/rules/kb-programme.mdc` says lot 0 "rewrites those files; until it is merged, this rule wins". After the merge that sentence is history. Do you want it trimmed in a later lot, or left as a record?

## Next

`docs/SESSION-KB-LOT-01-Store-and-migrations.md`, tab "KB 1 - Store and migrations" (branch `feat/kb-store`); lot 2 `docs/SESSION-KB-LOT-02-Names-accents-and-sounds.md` (branch `feat/kb-normalize-phonetic`) may start with it. Recommended first if only one chat is opened: lot 1. Prerequisite: this lot merged into `kb/integration`. What lot 1 must know: `knowledge/mod.rs` exists and only declares `pub mod diagnostics;`; `IndexSummary` gained `timings: Option<AnalysisTimings>` (anything that builds an `IndexSummary` literal must set it); `AskAnswer` gained `diagnostics`; the language guard above applies to every new Rust file, fixtures included.

## Proposed patch for `AGENTS.md` (the owner applies it)

"Current objective - v0 prototype" currently ends with the two dated milestones. Proposed replacement of that paragraph:

```text
## Current objective — v0 prototype and the Knowledge Base

One goal: a reliable installable prototype on the pilot GP's workstation, with the AI server (Ollama behind the gateway) on a Mac mini **at her practice**. Milestones are capabilities, not dates (8 October 2026): the chain holding end to end on fictional fixtures (30 September 2026) is reached; **the 14 October 2026 milestone is withdrawn**; the Windows installer and the work on real documents are their own track, gated by the DPIA draft, a named data controller and disk encryption. The product now grows a local, source-grounded Knowledge Base, in lots whose milestones K-A to K-E are in `docs/ROADMAP.md` and whose decisions are in `docs/DECISIONS.md`. Given a choice between one more impressive feature and making the existing flow more reliable, private, testable and replaceable, choose the latter. Do not rewrite what works.
```

and in "Out of scope for now", the line "Frozen for the v0 sprint: Open WebUI development, voice, vision, certificates, referral letters, mobile, app stores, RBAC, billing, analytics, large model catalogues, commercial packaging." becomes:

```text
- Frozen: Open WebUI development, vision, certificates, referral letters, mobile, app stores, RBAC, billing, analytics, large model catalogues, commercial packaging.
- Planned, each behind its own decision when its turn comes (8 October 2026): voice (workstation-local, free licences only), file organisation, a calendar (local adapter first; Google needs a separate owner decision and the DPIA), external connectors. Cloud speech stays forbidden.
```

"Native mobile app, cloud speech" in the first bullet becomes "Native mobile app, cloud speech (forbidden)".

## Git

```text
git status
git add .cursor/rules/v0-sprint.mdc docs/DECISIONS.md docs/ROADMAP.md docs/RETRIEVAL.md docs/OPERATIONS.md docs/test-reports/knowledge-base-pass-1 fixtures/kb-baseline apps/desktop/src-tauri/src apps/desktop/src-tauri/tests/analysis_timings.rs apps/desktop/src-tauri/tests/embedding_parallelism_bench.rs apps/desktop/src/lib/ipc.ts apps/desktop/src/components/SettingsDialog.tsx apps/desktop/src/locales/fr-FR.json apps/desktop/src/locales/en-US.json
git commit -m "feat: add retrieval timings and scope the lexical search"
git push -u origin feat/kb-measure-and-unfreeze
gh pr create --title "feat: add retrieval timings and scope the lexical search" --body "Lot 0 of the Knowledge Base programme. Governance: the decisions of 8 October 2026 in DECISIONS.md, capability milestones K-A to K-E in ROADMAP.md (the 14 October milestone is withdrawn), and the always-applied v0-sprint rule no longer freezes the planned work. Measurement: every question and every Analyse pass is timed as numbers and machine codes only (no question, file name or passage), returned with the answer and appended to two JSONL files when write_timing_log is on (off by default); nothing is shown in the interface. Fixes that need no KB: the lexical bonus is computed among the scope's own files, and the full-text query runs once for an 'each document' question. Ranking, caps and the four-character term rule are unchanged. Adds the baseline measurement template, a generator for its fictional corpus, the human test, and an Advanced group in the settings dialog with the two measurement switches." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge feat/kb-measure-and-unfreeze --merge
git switch kb/integration
git pull
git tag kb-after-lot-00
git push origin kb-after-lot-00
```
