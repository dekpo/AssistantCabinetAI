# TABULAR step 06 - Explain and analyse a calculation (23 October 2026)

Branch `feat/explain-and-analyse-calculation`, from the tip of `feat/tabular-integration`. Decisions: `docs/DECISIONS.md`, "Taken while building TABULAR step 06". Plan: sections 2 (decisions 11 and 12), 4.3, 4.6 and 4.7, each completed with an "As built by step 06" paragraph.

## What changed

Under a computed figure the one button ("Demander a l'IA", which made the model read the question again) is replaced, for answers that carry a figure, by two actions:

- **"Expliquer le calcul"**: the program computes the figure again (no model), builds a calculation record, and one gateway call explains the method. The explanation is added under the figure; the figure, the "Understood as" line and the "computed without the AI" line stay.
- **"Demander a l'IA"**: the analysis. The writer of the mixed tier gets the record in its `Table results` block and `ANALYSIS_ADDENDUM` in its instruction; with documents ticked it crosses them, with tables only it is the same without documents.
- For an answer with no figure (a nudge) the button keeps its old meaning.

| File | Change |
| --- | --- |
| `src-tauri/src/calculation_follow_up.rs` (new) | `CalculationFollowUp`, `ShownCalculation`, `recompute`, `explain`, `Explanation`, `EXPLAIN_INSTRUCTION` |
| `src-tauri/src/tabular/escalation.rs` | `CalculationRecord`, `format_calculation_record` (+ 9 tests, one per field) |
| `src-tauri/src/tabular_answer.rs` | `SheetSize`, `sheet_sizes`, `calculation_record`, `replay_model_plan`; `force_model` no longer applies to a figure |
| `src-tauri/src/mixed_answer.rs` | `ANALYSIS_ADDENDUM`, `CalculationSide`, `MixedContext.calculation`, `build_turn_for`, `table_evidence_text` (`pub(crate)`, record + sizes), analysis lifts the zero-call early return |
| `src-tauri/src/commands.rs` | `calculation_follow_up_tier`, new `ask_with_sources` parameter, `AskAnswer.explanation`, `advisory_tier` (figure kept), `mixed_tier(.., known_figure)` |
| `src-tauri/src/workbook_use.rs` | `WorkbookUseHow::Explained`, `Answered::Explained` |
| `src/lib/calculationFollowUp.ts` (new) | `buttonsFor`, the explanation helpers, `StoredCalculation`, `explanationBlock` |
| `src/lib/ipc.ts`, `src/state/useChat.ts`, `src/components/MessageList.tsx`, `ChatPanel.tsx` | options object, `explainCalculation`, `analyseCalculation`, the block, the buttons |
| `src/lib/annotations.ts`, `src/lib/workbookUse.ts`, `src/speech/output/readingParts.ts` | copy, trace line and read-aloud of the explanation |
| `src/locales/fr-FR.json`, `en-US.json` | 7 keys each (`actions.explainCalculation`, `actions.analyseCalculation`, `chat.explanation*`, `workbookUse.explained`) |

## Tests

Automatic tests added: `tests/calculation_follow_up.rs` (18, run alone with `--test-threads=1`), 9 in `escalation.rs`, 3 in `mixed_answer.rs`, 1 in `workbook_use.rs`, 3 in `calculation_follow_up.rs`, `src/lib/calculationFollowUp.test.ts` (23). Existing tests changed on purpose because they asserted the old meaning on a figure: `tests/tabular_query_plan.rs` (`forcing_the_model_on_a_figure_no_longer_re_reads_the_question`), `tests/tabular_hap1.rs` (`ask_ai_on_a_figure_keeps_the_computed_value_and_does_not_ask_the_model`). Final pass (once, after the last edit, 23 October 2026): `cargo test -j 2 --no-fail-fast -- --test-threads=1` 2182 passed across 91 test binaries (one frozen assertion of `tests/mixed_routing_baseline.rs`, case B, updated on purpose: the table block is now the calculation record, and that file then passes alone, 27 of 27); `cargo clippy --all-targets` shows no warning in the lines of this step (the 40 reported are in code this step did not touch); `pnpm run build` green; `pnpm run test` 116 files, 2909 tests passed. Pre-check before the first edit: 99 lib tests of the touched modules, all passing.

## Walked on a copy of her index (23 October 2026)

Settings read: Documents `C:\Users\elise\AssistantCabinetAI-test\Docs`, Tables `C:\Users\elise\AssistantCabinetAI\Data`, server `http://localhost:8080`, model `qwen3:1.7b`, `tabularRowsMode` `sample`, knowledge mode `suggest`. The index was copied into the scratchpad and the real `prepare` and `calculation_record` run on the copy (a throwaway test, deleted); the original was never opened.

`Test/factures-fournisseurs-2026.xlsx` (sheet Factures, columns date, fournisseur, montant, 8 data rows, header row 1; suppliers Fournitures Dupont, MedSupply, Papeterie Lefevre):

| Question (fr-FR) | Figure | Rows kept | Understood as |
| --- | --- | --- | --- |
| Quel est le montant total ? | 2215 | 8 of 8 | - |
| Quel est le montant total en mars 2026 ? | 660 | 3 of 8 | date in month 3; date in year 2026 |
| Quel est le montant total de MedSupply ? | 1450 | 3 of 8 | fournisseur = "MedSupply" |
| Quel est le montant total de MedSupply en mars 2026 ? | 380 | 1 of 8 | fournisseur = "MedSupply"; date in month 3; date in year 2026 |

Found, pre-existing, not investigated (DECISIONS N-8, N-9): a « du 01/03/2026 au 31/03/2026 » range is read as no filter (2215, no "Understood as" line); « Combien de fournisseurs distincts ? » is read as a count of rows.

## Corrections made to later prompts
## Owner's test of step 06 (23 October 2026)

Parts A to E and G as expected, with: a stale `explained` line with the AI stopped (fixed), English lines of the record copied into the French explanations (one instruction sentence added, not measured), `CHF` invented by the model in E3 (DECISIONS N-12), and in F2 the button under a refusal returning the same refusal with no model call when a document is ticked (fixed: DECISIONS step 06, row 13). Mini test of that fix: `docs/SESSION-TABULAR/reports/step-06-correctif-nudge-mixte-test-humain.md`. After the fix: `cargo test --test calculation_follow_up` 18 of 18, clippy clean on the touched lines; the full suite was not rerun for these three small changes.

## Corrections made to later prompts

Prompts 07, 08, 10 and 11 each got a section "What step 06 changed for this step" (the new `MixedContext` field in its eight places, `build_turn_for`, `table_evidence_text` and the record function names, `mixed_tier`'s ninth parameter, the options object of `askWithSources`, the carry-over that step 11 must store and send back for explain/analyse to work on a follow-up). The plan's sections 4.6 and 4.7 got an "As built by step 06" paragraph each.
