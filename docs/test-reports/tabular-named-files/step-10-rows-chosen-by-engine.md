# TABULAR step 10 - Rows chosen by the engine

Branch `feat/engine-chosen-rows`, from `feat/tabular-integration` after step 09. 23 October 2026. Decisions: `docs/DECISIONS.md`, "Taken while building TABULAR step 10 (23 October 2026), to confirm". This file records what changed and what was measured.

## What changed

- **New pure module `tabular/rows.rs`**: `build_rows`, `assemble`, `RowsEvidence`/`RowsBlock`, `ROWS_INSTRUCTION`, `TABLE_ROWS_SHARE_CHARS` = 1 400, `rows_budget_chars`, `with_chosen_rows`. A partial selection is a labelled sample plus the engine's profile of all matching rows.
- **`tabular_answer.rs`**: `question_table_data` (`question_samples` is now a wrapper), `select_rows_for_writer`, `rows_of_workbook`, `wants_rows_for_writer`, `RowsAsk`, `RowsSource`, `TableData`, `calculation_record_listing`. A workbook the question names wins: only its rows are sent.
- **`tabular/engine.rs`**: `matching_row_indices`, `numeric_total_over`, `describe_filter` made `pub(crate)`; `question.rs`: `residual_for_rows`; `sample.rs`: cleaning helpers `pub(crate)`, `SampleRequest.rows_chosen_in`, `WorkbookSampleUse.rows_truncated`; `escalation.rs`: `CalculationRecord.rows_listed`.
- **`mixed_answer.rs`**: `MixedContext.rows` (eleven literals), `MixedAnswer.rows_sent: Vec<RowsEvidence>`, `build_turn_for(.., sample, rows, table_block, ..)`, number checks include the rows block, `entity_link` `pub(crate)`.
- **`commands.rs`**: `mixed_tier` and `advisory_tier` ask for rows when the setting is `auto`, the question writes or analyses, and no figure is being analysed. `settings.rs`: `TabularRowsMode::Auto`, default.
- **Interface**: third option in the Settings selector, `lib/rowsSent.ts`, a line under the answer (read aloud with the cautions), a disclosure with the exact rows, copy with the sources only, `WorkbookUse.rowsTruncated`, both catalogues.

## Tests

- `tests/engine_chosen_rows.rs` (13, run alone, `--test-threads=1`): exact rows, row numbers, engine totals and absence of other rows; month filter and figure checks; small, default and large windows (partial, whole, none); too many rows; `off` and `sample` unchanged; a calculation and a mail merge never send rows; hostile cell and masked identifier; entity from excerpts; no value behind a word; two values of one column; named workbook wins; figure plus rows record line.
- Unit tests: `tabular::rows` (6), `the_shares_fit_the_default_window` (with rows), turn position and byte-identity without rows, record line, settings default, trace flag; vitest `rowsSent.test.ts` (7), Settings dialog option.
- Pre-check before the first edit: 361 lib tests of `tabular::`, `mixed_answer::`, `settings::`, `workbook_use::`, `tabular_answer::` green.

## Final pass

Run once after the last big edit: `cargo test -j 2 -- --test-threads=1` exit 0, 2273 passed, 59 ignored; `cargo clippy --all-targets`: one warning of this step (`rows.rs`, fixed afterwards), the others pre-existing; `pnpm run build` ok; `pnpm run test` 2937 passed. Two small edits followed (named-workbook rule, `total < MIN_ROWS`): lib tests (1336) and the integration tests `engine_chosen_rows`, `mixed_routing_baseline`, `tabular_sample`, `mixed_answer`, `named_file_wins`, `calculation_follow_up` rerun green.

## Measured (her real state, a copy of her index, her tower)

Windows published by the gateway on 23 October 2026: `qwen3:1.7b` 4 096, `qwen3:4b-instruct` 8 192, `ministral-3:3b` 16 384 tokens. Rows share: 466, 1 400, 3 266 characters.

- `Test/factures-fournisseurs-2026.xlsx`: MedSupply -> rows 2, 4, 7, total 1450 (all three models); March -> rows 7, 8, 9, total 660; `fournisseur = Fournitures Dupont` (two words) -> rows 3, 6, 9, total 550.
- `factures-test.csv`: `montant > 50` -> rows 7 to 10, total 300.
- `revenue_sub_agency.csv`, year 2024 (15 958 rows): `ministral-3:3b` 16 rows, `qwen3:4b-instruct` 5, `qwen3:1.7b` none (the share cannot hold three rows). A question with the identifier `R03E` is a merge request (door 1) and sends no row.
- With everything ticked, before the named-workbook rule, an implied workbook (`kb-lot09-data/factures.csv`) also sent its MedSupply rows; now only the named workbook does.

## Corrections made to later prompts

Prompt 11 (where the carry-over plugs into the rows selection, new names, eleven literals); prompt 12 (final names and the 1 400 share, blocks of the turn); plan 4.5 and section 8, points 2 and 9.
