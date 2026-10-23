# TABULAR step 07 - Checked formula help (23 October 2026)

Branch `feat/checked-formula-help`, from the tip of `feat/tabular-integration`. Decisions: `docs/DECISIONS.md`, "Taken while building TABULAR step 07". Plan: section 2 (decision 13), 4.1 and 4.4 (the latter completed with an "As built by step 07" paragraph).

## What changed

Every formula a model writes is now read by the program and the result is shown beside the answer; the model's text is never edited. A function name or a column letter that does not exist triggers one more request to the model.

| File | Change |
| --- | --- |
| `src-tauri/resources/formula-functions/excel/{fr-FR,en-US}.json` (new) | 257 functions each: canonical name, local name, argument separator, decimal mark, `reaches_outside` |
| `src-tauri/src/tabular/formula_check.rs` (new) | `extract_formulas`, `check_formula`, `check_answer`, `FormulaContext`, `FormulaIssue`/`FormulaNote`, eleven machine codes, the dictionaries and their loader (23 unit tests) |
| `src-tauri/src/tabular/card.rs` | `WorkbookCard.shape` (`WorkbookShape`, `SheetShape`, `shape_of`): the structure the text describes, whole (3 tests) |
| `src-tauri/src/mixed_answer.rs` | `MixedContext.formulas`, `MixedAnswer.{formula_notes, formulas_verified, formula_repaired}` (`Default`), the check and the single repair in `answer()`, `FORMULA_REPAIR_INSTRUCTION`, `repair_turn`; one sentence added to `MIXED_INSTRUCTION` (3 tests) |
| `src-tauri/src/settings.rs`, `lib.rs` | `SpreadsheetApplication` and the setting `spreadsheet_application` (`excel` by default; no selector, one value) |
| `src-tauri/src/commands.rs` | the setting is read and `FormulaContext::new` called in `mixed_tier` and `advisory_tier` |
| `src/lib/ipc.ts`, `src/state/useChat.ts`, `src/lib/annotations.ts` | `FormulaNote` types, the entry fields, `formulaNoteLine`, the quiet verified line and the repaired line |
| `src/locales/fr-FR.json`, `en-US.json` | `chat.formulaIssue.*` (12 sentences), `chat.formulaVerified`, `chat.formulaRepaired` |
| `tests/formula_check_wiring.rs` (new), `tests/common/counting_gateway.rs` | 10 wiring tests against the counting gateway (`start_gateway_replying_in_turn` scripts one reply per request) |

## Tests

Added: 23 in `formula_check.rs`, 3 in `card.rs`, 3 in `mixed_answer.rs`, 1 in `settings.rs`, 10 in `tests/formula_check_wiring.rs` (run alone with `--test-threads=1`), 8 in `src/lib/annotations.test.ts` (the eighth after the owner's test). The eight `MixedContext` literals of the existing test files got `formulas: None` (and nothing else). Pre-check before the first edit: the 22 `mixed_answer` and 19 `card` unit tests, all passing. Final pass (once, after the last functional edit, 23 October 2026): `cargo test -j 2 --no-fail-fast -- --test-threads=1` **2222 passed, 0 failed, 59 ignored across 92 test binaries**; `cargo clippy --all-targets` shows no warning in the lines of this step (two `manual_pattern_char_comparison` warnings in `formula_check.rs` were fixed after the pass, then `cargo clippy --lib` showed none for the touched files and the 23 unit tests were run again; the other warnings are in code this step did not touch); `pnpm run build` green; `pnpm run test` **116 files, 2916 tests passed**. `rustfmt --check` blocks (before -> after): `mixed_answer.rs` 9 -> 9, `card.rs` 1 -> 1, `settings.rs` 1 -> 1, `commands.rs` 0 -> 0; the two new Rust files are clean.

Routing verified first, as the prompt asked: on a copy of her index, "Ecris une formule qui additionne la colonne montant", "Explique la formule de la colonne total", "Write a formula that adds base and total in calculs.xlsx" and five other formula questions are advisory in tables-only (`advisory_target`), return `None` from `prepare_if_data_only` (door 3) and the no-table-part nudge from `prepare_for_mixed` (door 4): step 05 left no gap for this step. The probe was a throwaway test, deleted.

## Walked on a copy of her index (23 October 2026)

Settings read: Documents `C:\Users\elise\AssistantCabinetAI-test\Docs`, Tables `C:\Users\elise\AssistantCabinetAI\Data`, server `http://192.168.1.83:8080` (the tower was not reachable from the work machine), model `qwen3:1.7b`, `tabularRowsMode` `sample`, locale `fr-FR`. The real cards and the real checker were run on the copy (a throwaway test, deleted): `Test/factures-fournisseurs-2026.xlsx` (sheet Factures: A date, B fournisseur, C montant) and `Test/rdv-mars-2026.xlsx` (sheet RDV: A date, B salle, C patient, D duree_min). Checked on those cards: `=SOMME.SI(B:B;"MedSupply";C:C)` verified; with the factures card alone `...;D:D)` is `unknown_column_letter` (D, last C), with both cards D is accepted and E is flagged (last D); `SUMIF` in French is `function_in_other_language` (expected `SOMME.SI`); commas in French and semicolons in English are `wrong_separator`; `'rdv-mars-2026'!A:D` is `file_used_as_sheet` with both cards and `unknown_sheet` with one; `RDV!A:D` is accepted with both; `WEBSERVICE`/`SERVICEWEB` are `reaches_outside` (plus the other-language note for the name that is not the user's); `[TVA]` is `unknown_column_name`. The four formulas the 22 October models wrote (`SOMMEIFS`, `JOIN`, files used as sheets) are unit tests of `formula_check.rs`.

The pilot-level behaviour with a real model (what `qwen3:1.7b` writes, how often it needs the repair, whether the new instruction sentence stops the copying of English card text) was **not measured**: the AI machine could not be reached. The human test script asks for exactly that.

## Corrections made to later prompts

- Prompt 08: a section "What step 07 changed for this step" (the tenth place of `MixedContext`, `MixedAnswer` is `Default`, `WorkbookCard.shape`, a formula question may make two gateway calls).
- Prompt 15: a section "What step 07 built for this step" (the checker, the dictionaries and their possible `volatile` list, `explain` left open, the word lists), item 7 itself unchanged.
- Prompt 20, item 5: the real folder layout and the setting name.
- Plan 4.4: an "As built by step 07" paragraph.
- Prompts 12 and 14 mention the checker only in passing: read, nothing to change.

## Owner's test (23 October 2026) and what it changed

Played on her index with `qwen3:1.7b`. Parts A, C, D and G as expected; no false note anywhere. In B and E a correction happened (`=SOMME(C2:C9)` for the MedSupply question, with the filter lost; `=RECHERCHEV(Factures!A2, RDV!A:D, 3, FALSE)` with a true separator note), but the first version had been replaced and **the correction could not be judged**. Fixed: `MixedAnswer.formula_repaired_from` and the lines « Avant la correction : ... » (DECISIONS row 16). In F the model refused with two workbooks and wrote `=SUM(C2:C9*(B2:B9="MedSupply"))` with one (« Formula checked », true). The `reaches_outside`, bracket-name and invented-column notes were not produced by the model in this run.

Checks after the fix, run on the touched targets only (the full suite had passed before this small change and was not rerun): the 10 wiring tests alone, the `mixed_answer` unit tests, `cargo build --tests` (every test target compiles), the full `pnpm run test` and `pnpm run build`.

Mini human test of the fix: none asked for; the lines appear on the next correction in any formula question (B1 of the script again).
