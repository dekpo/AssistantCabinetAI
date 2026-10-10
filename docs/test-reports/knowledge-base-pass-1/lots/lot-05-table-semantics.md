# Lot 5 - Entities from tables

11 October 2026 · branch `feat/kb-table-semantics` · based on `kb/integration` at `11af59e` (tag `kb-after-lot-04`) · tab "KB 5 - Entities from tables"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

Pressing Analyse on the Data card now teaches the knowledge base what the columns of each spreadsheet mean and which people, organisations, identifiers, places and items their values name: a column called `patient`, `fournisseur`, `Nom` + `Prenom` or `Email` is recognised from its header and checked against its values; a `montant` or a date never becomes an entity; each name keeps the sheet, the column, its first row and up to three sample rows, and two role columns of one sheet state a relation. A module only adds meaning and never changes a type (with the French health module off, the `patient` column is simply not recognised and the suppliers stay organisations), a personal identifier is hashed exactly as for a document, and the DATA engine is untouched: the stored inventory is byte for byte the one a pass without the knowledge base stores. Nothing runs at launch or in the background; the documents the new names concern are read again from their stored text at the end of the Data Analyse the user started, with no request to the AI. The owner ran the human test on 11 October 2026 (every step matched) and answered the open questions; the changes they asked for (rows as in the spreadsheet program, headers read in both languages, columns that were not read recorded with the reason, an order scheme) are in this lot and described under "Owner answers" below.

## What changed

**Rust, new (`src-tauri/src/knowledge/`)**

- `extract/table.rs` - the `TableEntityExtractor` and its analyzer: header words against the packs (exact, or contained with same-kind words), the shipped vocabulary check that keeps a module from changing a type, the value-shape guards, the candidates (one per distinct value), the pairing of a given name and a family name, the relations, the personal identifiers on a person's row. 6 unit tests.

**Rust, existing files (additive unless said)**

- `knowledge/ingest.rs` - `KnowledgeContext::store_workbook`, `build_workbook`, `data_summary`, the Data tally, `workbook_input` (a parsed workbook as columns of text), `DeltaBuilder::place_table` and `add_cells_mention` (the document builder's resolution and creation code reused, not copied), `place_identifiers` returns where each identifier went and takes the method, `add_mention` takes a role, the vocabulary epoch. 1 unit test.
- `knowledge/mod.rs` - `ColumnSemanticsDraft`, `KnowledgeDelta.column_semantics`, `KnowledgeSummary.{semantic_columns, workbooks_analysed}`.
- `knowledge/store.rs` - `apply_column_semantics` (a diff, like the other tables), `column_semantics_for`, `source_row`, `relation_count_for_sources`; `SourceEntityCounts.others` and `distinct` limited to people and organisations. The `kb_column_semantics` table itself exists since lot 1.
- `knowledge/extract/mod.rs` - `ColumnKind`, `ColumnInput.{kind, formula_cells}` and `ColumnInput::text`, `SheetInput.{has_header, row_numbers}`, `CandidateRelation.occurrence_count`.
- `knowledge/packs.rs` - `header_words` (camel case aware), a precomputed form of the stop-words. 1 unit test.
- `knowledge/gazetteer.rs` - `is_listed_type` (people and organisations only). 1 unit test.
- `knowledge/resolve.rs` - `InMemoryLookup` answers by hash lookup instead of scanning every draft (see Measured).
- `index_store.rs` - `put_tabular_inventory_with_knowledge` (the inventory and the delta in one transaction, the delta in a savepoint; `None` forgets the source).
- `tabular/mod.rs` - `build_inventory_with_workbook` and `build_inventory_with_source_rows`; `build_inventory` delegates. `SourceRows`: for each sheet the row of the file every row came from.
- `tabular/csv_adapter.rs`, `tabular/xlsx_adapter.rs` - `open_with_source_rows` (the trait's `open` delegates to it, so the engine is unchanged): the line each CSV record starts on (counted from the bytes) and the XLSX range's first row.
- `data_folder.rs` - `analyse_with_knowledge`; `analyse` delegates with `None` and keeps its behaviour (its progress events included, which a test pins).
- `commands.rs` - `index_data_folder` builds the context and passes it.

**Packs and resources**

- `health-fr` (both languages): a column of national social security numbers. `base` (both languages): header entries for an order number and an `order_number` pattern that accepts `CMD-2026-001` whole; `base/fr-FR`: `unknown` among the stop-words. `resources/README.md`: how a header is read.

**Interface (`apps/desktop/src`)**

- `lib/ipc.ts`, `lib/knowledgeSummary.ts` (lines for columns, spreadsheets and links; "files" instead of "documents" for a Data pass), `components/DataFolderCard.tsx` (prints them, labels the progress while documents are read again), `locales/{fr-FR,en-US}.json` (13 keys each). 4 vitest tests.

**Tests**

- `tests/knowledge_tables.rs` - 47 integration tests (+ 3 ignored: the extraction benchmark, a printer of what a pass learns from the pilot's workbooks, an index writer for the script). `tests/common/knowledge.rs` - `Lab` gains the Data Folder, with `column_reading`, `relations`, `attributes_of` and `cells_mentions_of`.

**Documentation and human test** - see below. `human-tests/lot05_helper.py` (new), `human-tests/show_kb_entities.py` (extended: columns, table mentions, relations).

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 5". The ones to know:

| Decision | Reason |
| --- | --- |
| A header is read as a whole first; a header that merely contains a vocabulary word counts only when its other words are particles or words of headers of the same entity type | KBD-01's lesson, and the owner's rule: `statut client` is nothing, `nom caisse` is never a person because "nom" is a person's word |
| **A module never changes a type, for tables**: a header is also looked up in the vocabulary of every shipped pack; if the two readings give different types the column is left alone; a column only a module reads is **not classified** when the module is off | Owner rule of 11 October. Proved by an exhaustive test (every header of every pack, both languages, the sixteen combinations of the optional packs) and an end-to-end one |
| A given-name column and a family-name column make one person per row; neither makes an entity alone | A surname alone creates nobody (owner, 10 October); the pilot's mail-merge file has exactly this layout |
| Entities from a column are **active** at once, no candidates | The column is the evidence (master section 6, rule 5) |
| One mention per entity, sheet, column and role; rows counted **from 1 among the data rows**, first row and at most three samples; method always `column_value` | No per-cell rows (master section 5.2); KBD-13 is not copied |
| A personal identifier on the row of a person becomes an attribute `identifier:<scheme>` (the hash) only when the sheet has exactly one column of people | With two person columns the table does not say whose number it is |
| Only sheets the engine would answer from, with a header row, are read; a formula cell is blank and counted; a date column is not even copied | One notion of "a table" for the engine and the knowledge base; a formula's result is a claim |
| What documents are scanned for: people and organisations only (`is_listed_type`) | A city or an article name is an ordinary word of every document; the list is built with the same rule at the start of a pass and while it runs, which keeps a second pass from raising the epoch for nothing |
| A workbook is read again when its content, the extractor or the **vocabulary** changed, not when the list of names grew | A table does not depend on the list; `kb_meta.vocabulary_epoch` |
| The knowledge step is a savepoint, never a rollback of the inventory | Owner rule of 9 October (replaces the lot file's "rolls both back") |

## Deviations

- **Confidence of a header.** The lot file says 0.97 / 0.85 / 0.50. A pack entry already carries a confidence, so the exact reading is the entry's own number and the contained one 0.9 of it; the third tier (a single generic word in a longer header, 0.50) is below the 0.60 acceptance line and is the same as no match, so it is not computed. A column under the line is **recorded with the reason it was not read** (the owner's answer 3), which the lot file did not ask for.
- **Method code.** The lot file wrote `method='column_semantics'`; the `Method` enum of lot 3 already has `ColumnValue` (`column_value`), which is what is stored.
- **Atomicity.** "A failure after the inventory write rolls both back" is replaced by the owner's later rule (savepoint, inventory committed, workbook left due, counted in `errors`), tested by `a_failing_knowledge_step_never_costs_the_workbook`.
- **`CandidateRelation` gained `occurrence_count`**, so the four struct literals in the `extract/mod.rs` tests got one field: a compile change, no assertion moved. `ExtractedKnowledge` is unchanged; the table extractor's extra outputs (column readings, attributes) come with `TableReading`, and the trait implementation returns its `knowledge`.
- **Files beyond the lot's list:** `ingest.rs` (the builder), `commands.rs`, `gazetteer.rs`, `resolve.rs`, `packs.rs`, `store.rs`, `extract/mod.rs`, the locales and the TypeScript files, the two scripts. The lot file lists `data_folder.rs` and `tabular/mod.rs` for the wiring; the builder had to be reached.
- **Two lot 3 pieces changed for speed, with no change of meaning:** `InMemoryLookup` (the layer that shows the resolver a document's own drafts) scanned every alias of every draft per question, which is quadratic once a table drafts thousands of entities (13.9 s for the benchmark below); it now answers by hash. `PackSet::is_stop_word` normalised every stop-word per call; it compares a precomputed set.
- **Gazetteer rule (lot 3 code).** `Gazetteer::rebuild_from_store` took every non-identifier entity; it now takes people and organisations only. For documents, which only ever make those, nothing changes; for tables it keeps items and places out of the scan of letters.
- **Not tested:** XLSM and XLS (the adapter reads them; the lot file said "if the adapter supports them").
- The lot file's `src/tabular/mod.rs` and `data_folder.rs` descriptions match the code (the parsed `Workbook` was dropped by `build_inventory`, as the master says). No discrepancy found with the master's "what the code looks like today".

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a question produces reaches the store | Nothing in this lot reads a question |
| I4 - a lookup error means fall back | `a_failing_knowledge_step_never_costs_the_workbook` (a dropped `kb_` table: both workbooks still analysed, `errors: 2`, no half-written rows); a failed read inside `place_table` makes the whole workbook a bypass |
| I5 - the engine is not altered; `sheet_restrictions` never written | `the_inventory_is_the_same_with_the_knowledge_base_on_and_off`; `the_knowledge_layer_never_writes_sheet_restrictions` (grep over `knowledge/` and `data_folder.rs`); the whole `tabular_*` and `mixed_*` suites pass unchanged |
| I6 - same relative path in both folders | `the_same_relative_path_in_both_folders_stays_two_sources` |
| I8 - nothing the base holds reaches a model or a log | No prompt, log line or gateway message added; `the_summary_reports_counts_and_no_name`; `a_personal_identifier_column_is_never_written_as_itself` (every text value of every `kb_` table searched for the numbers) |
| I9 - model-facing strings stay neutral | No model-facing string added; no guard to add |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the source guard scans `table.rs` and every other file touched; it caught two French examples in my comments, now English); catalogue parity |
| I11 - no destructive action on files | Nothing here touches a file in either folder |
| I13 - no `cfg`, no Windows path literal | No `cfg`; tests build paths with `Path::join` and `CARGO_MANIFEST_DIR`. **Not run on macOS** |
| I14 - a second pass writes nothing | `a_second_pass_over_unchanged_workbooks_writes_nothing_and_reads_nothing` (snapshot of every `kb_` table), and the same with a document present in `a_person_a_document_names_and_a_workbook_lists_is_one_entity` |
| The owner's rule on modules | `a_module_never_changes_the_type_of_a_column`, `a_module_never_changes_the_type_of_a_name_in_a_table`, `a_header_only_a_module_reads_carries_less_meaning_when_the_module_is_off_and_is_never_retyped`, `a_change_of_modules_reads_an_unchanged_workbook_again` |
| I1, I2, I7, I12 | Untouched: no scope, no query code in this lot |

## Verification

Windows 11, debug profile, `kb/integration` plus this lot. Baseline measured before the first change.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps/desktop/src-tauri`) | 986 passed, 0 failed, 10 ignored | **1042 passed, 0 failed, 13 ignored** (+56: 9 unit tests, 47 in `tests/knowledge_tables.rs`) |
| of which `--lib` (what CI runs) | 680 | 689 |
| `cargo clippy --lib` | 21 warnings (lot 4's figure) | **21** (one new, `too_many_arguments` on `add_mention`, allowed as the other builders are) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it), `data_folder.rs`, `tabular/csv_adapter.rs`, `tests/knowledge_tables.rs`, `tests/common/knowledge.rs`; `index_store.rs` (4 hunks), `commands.rs` and `tabular/xlsx_adapter.rs` (1 hunk, in a test) have the same hunks as on `HEAD`; no plain `cargo fmt` was run |
| `pnpm run test` (from `apps/desktop`) | 358 passed (28 files) | **362 passed** (28 files) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

Not run: the application window, a gateway, macOS, a release `tauri build`.

## Measured

Windows 11, the owner's development PC. For orientation.

| What | Number | Method |
| --- | --- | --- |
| The pilot's three fictional workbooks (two data files and the mail-merge file), names only | 119 ms | `elapsed_ms` of the summary, debug (`print_what_a_pass_learns_from_the_pilot_workbooks`) |
| A Data pass over 50 000 rows x 8 columns (7 300 distinct values), with the knowledge base / without | **3.9 s / 0.24 s** | `cargo test --release --test knowledge_tables extraction_cost -- --ignored --nocapture`. Reading the cells and judging them is **76 ms** of it (measured alone on 50 000 rows x 5 columns); resolving and teaching the 7 300 values is about 2.3 s and writing them about 1.4 s. Before the two speed changes of this lot it was 13.9 s. A real workbook of a few hundred rows is tens of milliseconds |
| 100 000 rows, two name columns | about 2 s | `a_hundred_thousand_rows_are_read_in_a_bounded_time`, debug |
| The exhaustive header test (every header, both languages, sixteen combinations) | 2.5 s | debug |
| Documents: extraction and resolution of 1 000 chunks | 102 ms (was 127 ms) | lot 4's benchmark, release, after the `InMemoryLookup` change |

For comparison with `tabular_column_cache_bench`: that benchmark measures the engine's cache hit; this one the extra cost of the knowledge base on top of a pass, which the `tabular_column_cache_bench` figures do not include.

## Not done

- Any use of the columns at question time (lot 6), any screen that lists them (lots 7 and 8).
- XLSM and XLS in the tests; a header in another script.
- The surname-then-initial form (KBD-15): decided on 11 October 2026 to be fixed in `fix/kb-surname-initial` before lot 6.
- No demotion of a promoted candidate (KBD-12).
- macOS.

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 5"), `docs/ARCHITECTURE.md` (the table path), `docs/PRIVACY-AND-SECURITY.md` (the lifecycle row, the identifier on a person's row), `docs/CLIENT.md` (the Data card's lines), `apps/desktop/src-tauri/resources/README.md`, `defects-register.md` (KBD-14, KBD-15, notes on KBD-01, KBD-12, KBD-13), this report, the human test and its two scripts. Not touched: `docs/RETRIEVAL.md` and `docs/SELECTION-AND-MEMORY.md` (lot 6), `docs/LANGUAGE-AND-LOCALE.md`.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-05-table-semantics.md` - **run by the owner on 11 October 2026: every step B1 to B11 and the smoke test S1 to S7 matched** (first version of the protocol; the screenshots and outputs are in the owner's message of that day). Two findings, both about the document: the commands were abbreviated (the owner typed `stop-ia` and ran `show_kb_tables.py` at B5), and the expectation "no error banner" with the AI stopped was wrong (the application's own connection banner shows). Both are corrected, and a rule was added (`.cursor/rules/kb-programme.mdc`, "Exact commands in a human test"). The document now in the repository is the version to **run again for the changes made after the owner's answers** (B2, B5, B12, B13). Originally: Visible in the application: **partly** (the Data card's new lines and its progress label; the columns and the names are read with `show_kb_entities.py`). Part A (5 minutes, no application): the printed summary and the columns read on the pilot's three workbooks, and the test file. Part B (about 25 minutes, the application): the first pass, no address in clear, a second pass that reads nothing, the French module off and on (nothing is retyped), then optionally a changed workbook, the AI machine stopped, a removed workbook, the Data Reset that leaves the documents alone, and the off choice. The smoke test is seven spreadsheet questions with known answers. Every file change is done by `lot05_helper.py`, which the agent ran on temporary folders (every command); every count quoted is asserted by `the_numbers_the_human_test_quotes_hold`. **The agent did not open the application window.**

## Owner answers (11 October 2026)

Defect rows: **KBD-14** (added, deferred) and **KBD-15** (added; **the owner chose to fix it before lot 6**, in `fix/kb-surname-initial`). Notes on KBD-01, KBD-12 and KBD-13. No row is `blocking`.

1. **KBD-15: yes, fix it in its own branch before lot 6.**
2. **Headers in both languages: yes.** Implemented: the active modules' column rules are read in every language the build ships; entries of different types at the same rank are a conflict and the column is left alone (`ambiguous_header`). Tests: `a_french_session_reads_english_headers_and_an_english_one_french_headers`, and the exhaustive `a_module_never_changes_the_type_of_a_column` still holds.
3. **Items and places stay out of the scan of documents: yes** (unchanged).
4. **Columns that were not read are recorded with the reason: yes.** Implemented: `no_header_match`, `module_off` (a hint with confidence 0), `ambiguous_header`, `number_or_date`, `no_values`, `values_mismatch`; `ColumnSemanticsRow::is_read`; the script prints both lists. Tests: `a_column_that_is_not_read_is_recorded_with_the_reason`.
5. **Rows as the spreadsheet program counts them: yes.** Implemented through the adapters (`SourceRows`), which meant touching the CSV and XLSX adapters, not the engine: the adapters' `open` delegates to a new `open_with_source_rows`. Found on the way: the CSV reader's own line numbers run short after a blank line, so the lines are counted from the bytes. Tests: `rows_are_the_rows_of_the_file_whatever_sits_above_the_table_or_between_its_lines` (a sheet whose header is on row 3, a CSV with a blank line).
6. **Order numbers: yes.** Header entries and an `order_number` pattern that accepts `CMD-2026-001`; `identifier/order`. Test: `an_order_number_column_is_read_by_its_header_and_its_numbers_are_orders`.
7. **A `Nom` column of company names: accept the limit**, correct by hand in lot 8.
8. **Upper-case names displayed as `Dupont Jean`: kept.**

## Next

**Before lot 6: `fix/kb-surname-initial`** (the owner's decision on KBD-15), a small branch from `kb/integration` after this lot is merged. **Lot 6 needs lots 4 and 5 merged** (lot 4 is; this one once its pull request is).

- `docs/SESSION-KB-LOT-06-Targeted-scope.md`, tab "KB 6 - Targeted scope", branch `feat/kb-targeted-scope`. What it must know from this lot:
  - Table mentions have `locator_kind='cells'` with `sheet`, `column_name`, `first_row`, `sample_rows` (the rows of the file, as the spreadsheet program counts them) and the column's `role`; `store::column_semantics_for(connection, Domain::Data, path)` says how each column was read; `store::source_row` gives a source's `kb_version` and epoch.
  - A table's entities are `active` from the first source (no candidate), so a name found in one workbook resolves above the 0.40 cap: `selectivity` matters more for tables than for documents.
  - `source_ids_for_scope` and the `(domain, relative_path, content_id)` mapping already cover workbooks.
  - Items and places are entities of types `item` and `location`; people and organisations are the ones the gazetteer knows.
  - KBD-15 (the surname-then-initial form) affects exactly the case lot 6 narrows on.
  - Source guard reminder: no French word or non-ASCII letter in `src/`, comments included (this lot tripped it twice, on "de la caisse" and on a French test sentence).

## Git

```text
git status
git add .cursor/rules/kb-programme.mdc apps/desktop/src-tauri/src apps/desktop/src-tauri/resources apps/desktop/src-tauri/tests apps/desktop/src docs/ARCHITECTURE.md docs/CLIENT.md docs/DECISIONS.md docs/PRIVACY-AND-SECURITY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: classify table columns and record their entities"
git push -u origin feat/kb-table-semantics
gh pr create --title "feat: classify table columns and record their entities" --body "Lot 5 of the Knowledge Base programme. The Data Analyse now reads what the columns of each workbook mean (from the header words and the packs, checked against a sample of the values) and records the people, organisations, identifiers, places and items their values name, with the sheet, column, first row and sample rows; two role columns of one sheet state a relation. A module only adds meaning and never changes a type: a header is also looked up in the vocabulary of every shipped pack and the column is left alone when the readings disagree. A given name and a family name in two columns are one person. Personal identifiers are hashed as for documents, and are attached to the person of the same row only when the sheet has exactly one column of people. The engine is untouched: the stored inventory is identical with the knowledge base on and off. The knowledge step is a savepoint, never a cost to the inventory. Workbooks are read again only when their content, the extractor or the vocabulary changed. The documents the new names concern are read again from their stored text at the end of the Data Analyse, with no request to the AI. Also: the draft lookup of the document builder answers by hash instead of scanning (quadratic for a table), the stop-word set is precomputed, and the scan of documents is limited to people and organisations. Data card summary lines and catalogue entries in French and English; a NIR column, an order-number column and unknown as a null-like word in the packs. Headers are read in the words of both shipped languages (a conflict between two readings leaves the column alone), the columns that were not read are recorded with the reason (including a module that is off, as a hint with no confidence), and rows are the rows of the file as the spreadsheet program counts them (the CSV and XLSX adapters report them through a new open_with_source_rows; the engine's open is unchanged)." --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-05
git push origin kb-after-lot-05
git switch -c feat/kb-targeted-scope
```
