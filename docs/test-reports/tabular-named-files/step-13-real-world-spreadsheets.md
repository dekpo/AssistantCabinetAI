# TABULAR step 13 - Real-world spreadsheets

Branch `fix/real-world-spreadsheets`, from `feat/tabular-integration` after step 12. 24 October 2026. Phase A (analysis) is `design/13-real-world-spreadsheets.md`; the owner approved its recommendations Q1 to Q6 the same day. Decisions: `docs/DECISIONS.md`, "Taken while building TABULAR step 13 (24 October 2026), to confirm".

## What changed

- **New pure modules** (no `tauri::`): `tabular/layout.rs` (`SheetFacts`, `SheetLayout`, `RowPolicy`, `prepare_sheet`: merged cells filled down, blank rows and notes, labelled total row, placeholders, hidden and filtered rows by policy, one sheet per table of a sheet that holds several, Excel Tables as exact regions) and `tabular/layout_read.rs` (the facts read from the XML of `.xlsx`/`.xlsm` and `.ods`, no new crate).
- **Inventory** (`tabular/inventory.rs`): `TabularInventory.facts`/`layout_version`, `SheetInventory.layout`, `ColumnInventory.error_count`, `build_view` (the one way production code builds what a question reads), `data_row_number`, `caveats`; a header-only sheet is an empty table; `looks_tabular` ignores hidden sheets.
- **Adapter and entry points** (`xlsx_adapter.rs`, `tabular/mod.rs`): `open_with_facts`; error cells written as the spreadsheet literal; `RawWorkbook`, `load_current_raw`, `view_of`; the open-password code.
- **Engine**: group, count-per-group and distinct labels trimmed and case-folded (commonest spelling shown); hidden sheets not reachable unless named (`reachable_sheets`, `nameable_sheets`); a figure with no sheet named picks the only table (`resolve_figure_sheet`; the mail merge does not).
- **Policy**: `DataFolder::row_policy`, `AppState.row_policy` read by `family_views_of`, `ask_with_sources(.., row_policy, ..)`, `resolve_row_policy`, `TableCarryOver.row_policy`; `load_workbook_cached` returns the view; rows acted on or sent are always read under `Exclude`; the structural row count follows the policy.
- **Visible**: `WorkbookUse.layout` (`LayoutNote`, `note_layout`), `CalculationRecord.layout`/`rows_left_out`, card facts and header row numbering, `FileRecord.layout_caveats` and the explorer marker, the button "Recalculer sans/avec les lignes masquées" (`rowPolicyButton`, `useChat.askWithRowPolicy`, `AskOptions.rowPolicy`), about thirty catalogue keys per language, `errors.password_protected`.
- **Data**: `total_words` and `empty_markers` in both question packs (read from every pack).

## Tests

- **Pre-check before the first edit: not run** (a deviation from the opening checklist: phase A had measured the starting point and no existing test was changed before the first new file).
- New: `tests/layout_real_world.rs` (18, real `.xlsx` built inline by the new `LaidOutSheet` writer of `tests/common/tabular_fixtures.rs`: hidden row and filter, merged cell, hidden sheet, total row, two tables, Excel Table, blank row and note, placeholders, header-only CSV, spellings, the Notes sheet, row numbers, caveats, and two tests through `tabular_answer::answer` and `workbook_use::note_layout`); `tests/layout_guard.rs` (1: the only files that reach the raw grid or an adapter); unit tests in `layout.rs` (11), `layout_read.rs` (6), `escalation.rs` (3), `card.rs` (1), `inventory.rs` (2), `tabular/mod.rs` (1), `commands.rs` (1); pack parity (1); vitest `workbookUse.test.ts` (+6) and `calculationFollowUp.test.ts` (+3). Golden numbers are derived by hand in the test comments (36 / 1 036; 31 / 1 005 / 5; 35 not 70; 30 / 300).
- One existing test changed on purpose: `a_single_row_sheet_has_no_header...` became `a_single_row_of_labels_is_a_header...` (header-only table, decided in the note).

## Final pass (once, after the last edit)

- `cargo test -j 2 --no-fail-fast -- --test-threads=1`: exit 0, **2 348 passed, 0 failed, 59 ignored**. (A first run stopped on `named_file_wins` > `a_workbook_narrowed_to_one_sheet_is_read_again_from_that_sheet`: the "only table" default I had put in `resolve_sheet_inventory` also reached the mail merge, which must stay ambiguous. It is now in `resolve_figure_sheet`, used by figures only; the whole pass was run again.)
- `cargo clippy --all-targets`: exit 0; the two warnings of `layout.rs` were fixed afterwards (`cargo clippy --lib` clean on the file, layout tests re-run); every other warning was there before.
- `pnpm run build`: ok. `pnpm run test`: **2 967 passed** (118 files).
- `rustfmt`: new files clean (`layout.rs`, `layout_read.rs`, `layout_real_world.rs`, `layout_guard.rs`: 0); no modified file has more `Diff in` blocks than at `HEAD` (checked by formatting each file from standard input, because `rustfmt` on a test file follows `mod common;` and rewrites the shared helpers).

## Measured on her copies (read only, nothing of hers touched)

- Test folder (510 spreadsheets): files entirely red because of a total formula **11 -> 0**; 31 sheets with a total row left out; the first column typed `date` in those 25 of 31 (6 before). Configured Data Folder (15 spreadsheets): no change, 0 red.
- Her index was not opened: the human test uses five files built for the occasion (`docs/SESSION-TABULAR/reports/step-13-files/`, git-ignored, not a fixture), and its counts do not depend on her index.

## Walk of the human test (question by question through `tabular_answer::answer`, on a temporary copy)

`ventes-simples` total 400; `journal-filtre` total 1 036 (36 with the hidden row left out), rows 5 (4), by category Deplacements 1 005 (5) and Fournitures 31; `deux-tableaux` asks which sheet (`Suivi A1:C3`, `Suivi A5:C7`), then 30 and 300; `classeur-feuille-masquee` rows 2, and 1 for `Calculs` named; `liste-avec-total.csv` total 35, rows 2. The words "feuille" or "sheet" in a question make it a question about the sheet names (existing rule), so the script names a table without that word.

## Owner's human test (24 October 2026)

Played on her real state (Data Folder `C:/Users/elise/AssistantCabinetAI/Data`, subfolder `etape-13`, tower on, `qwen3:1.7b`). Parts 1, 2 (1 036, then 36 after the click, group totals, the three layout lines) and 4.2 conformed. Findings:

| Part | Finding | Cause | Fix |
| --- | --- | --- | --- |
| 3.2 | A figure (30) instead of the question "which table?": "compris par le modele", sheet `Suivi A1:C3` | The hidden interpreter picked a table of the sheet | Not asked when several tables of one sheet are present and none is named; test `a_question_that_names_none_of_two_tables_is_asked_never_guessed_by_the_model` (red without the fix, green with) |
| 4.1 | `Combien de lignes de Calculs ?` answered 1 but said "1 hidden sheet is not read" | The note counted the sheet it had just read | `hidden_sheets_unread`; test `a_hidden_sheet_the_answer_was_read_from_is_not_reported_as_unread` |
| 2.4 | Deplacements 1 005 where the script said 5 or 1 005 | Not a defect: nine words, so no inheritance | Script wording |

After the fixes: the library tests and the tabular suites (`tabular_reference`, `named_file_wins`, `mixed_routing_baseline`, `table_follow_up`, `mixed_answer`, `engine_chosen_rows`, `layout_*`, pack parity) are green; the whole suite was not run again (the change is `skip_model` for a workbook with several tables of one sheet, and one count). Her restoration (part R) was not played yet: the app was closed after part 4.

## Corrections made to later prompts

Prompts 14, 15, 17, 18, 19, 22, 23 and 24 carry a "Corrected by step 13" section (written after the owner's go); 14, 17 and 24 were corrected again after the build: a table of a sheet that holds several is a sheet of its own, so there is no region field and no `AmbiguousRegion`. The plan has a section 10.
