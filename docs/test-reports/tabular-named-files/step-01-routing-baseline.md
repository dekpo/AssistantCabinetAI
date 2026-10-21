# TABULAR step 01: routing baseline tests

Date: 21 October 2026. Branch: `test/mixed-routing-baseline` (from the tip of `feat/tabular-integration`, `6fcf665`). No production behaviour changed.

## What changed

| File | Change |
| --- | --- |
| `apps/desktop/src-tauri/tests/mixed_routing_baseline.rs` (new) | 13 tests freezing how a question asked with documents and spreadsheets ticked together is routed today. One test per case of the plan's section 1.3, named `baseline_case_<letter>_...`, each tagged `// Frozen 20 October 2026.` and, for a defect, `DEFECT, changes in step NN`. |
| `apps/desktop/src-tauri/tests/common/counting_gateway.rs` (new) | The chat fake that counts requests and records the last turn of each: `start_counting_gateway()`, `start_gateway_replying(content)`, `CountingGateway { url, calls }` with `calls()` and `requests()`. Moved out of `tests/mixed_selection_routing.rs` (second file needing it). |
| `apps/desktop/src-tauri/tests/common/mod.rs` | `pub mod counting_gateway;` |
| `apps/desktop/src-tauri/tests/mixed_selection_routing.rs` | The local `CountingGateway` and `start_counting_gateway` removed; imports the shared one. Its five tests unchanged. |
| `apps/desktop/src-tauri/src/mixed_answer.rs` | Three golden tests of the exact turn `build_two_block_turn` builds: `the_turn_with_documents_and_no_table_result`, `the_turn_with_documents_and_one_figure`, `the_turn_with_no_documents_and_one_figure` (constant `FIGURE`). They interpolate `MIXED_INSTRUCTION`, so they freeze the block layout, not the instruction wording. |
| `docs/DECISIONS.md` | One section at the end: "Settled by the owner for the TABULAR series (20-21 October 2026)", decisions 1 to 30 (one row each) and 31 to 35 (second table), with the precision on decision 1. |

Workbooks are built inline at test time: `factures.csv` (60 rows, title row above the header, CP1252, `;`) and `agencies.csv` (2 400 rows) from `tests/common/tabular_fixtures.rs`, plus `commandes.csv` (ten orders with an identifier column) and `calculs.xlsx` (sixty rows, a formula in the first three cells of `total`, which keeps the sheet usable) written in the test file. The mail-merge template is a `.txt` in a temporary Documents folder. Nothing under `fixtures/`.

## The cases, as frozen

| Case | Test | Frozen behaviour | Flip |
| --- | --- | --- | --- |
| A | `baseline_case_a_...` | A named workbook plus an operation is answered by door 3: `Sum` 237 030 on `factures.csv` (French question), 270 599.5 on `agencies.csv` (English), zero gateway calls. | - |
| B | `baseline_case_b_...` | The writer receives exactly `Sheet agencies, column amount: sum = 270599.5 (computed over 2400 rows)` in the `Table results` block, and no `.csv` anywhere before the question. | DEFECT, step 03 |
| C | `baseline_case_c_...` | A named workbook without an operation: door 3 declines, `prepare_for_mixed` leaves `TryModel` with an unrecognised nudge (`skip_model` false); `mixed_answer::answer` gives `table == None`, `table_unavailable == NotAskedAbout`, exactly **one** gateway call (the writer; the hidden interpreter is not asked), and the turn ends with `Table results: none.`. English and French. | DEFECT, step 03 |
| D | `baseline_case_d_...` | "Which columns does agencies.csv have?" / "Quelles sont les colonnes de factures.csv ?" give `ColumnNames` from the inventory, zero calls. | - |
| E | `baseline_case_e_...` | An identifier found in a cell (`CMD-2026-002`) gives its row (`Rows`, `row_index == 1`), zero calls, documents ignored. English and French. | - |
| F | `baseline_case_f_...` | An operation plus a word matching no data ("for Nowhere"): door 3 declines; door 4 gets `TryModel` with a `ValueNotFound` nudge and `skip_model == true`; `mixed_answer::answer` returns the nudge with zero calls. | - |
| G | `baseline_case_g_...` (two tests) | Two workbooks ticked, none named, no column named ("How many rows are there?" / "Combien de lignes y a-t-il ?"): `WhichWorkbook` with both candidates. **Finding:** a question that names a column only one ticked workbook has ("What is the total amount?", `amount` is only in `agencies.csv`) is answered from that workbook without asking (`pick_target`, HAP-1 BUG-15). | - (kept) |
| H | `baseline_case_h_...` | "Write a letter for each invoice of factures.csv" (English and French) with two single-sheet workbooks ticked: `locate_fill_source(.., every_row = true)` is `None`; the control with only `factures.csv` ticked returns it (`key == None`) and `candidate_templates` finds the template. Door 4 then gives `table == None`, `NotAskedAbout`, one writer call, `Table results: none.`. | DEFECT, step 08 |
| I (table half) | `baseline_case_i_...` | A name no workbook carries ("contrat-2026.pdf"): `prepare_if_data_only` is `None`, `prepare_for_mixed` returns the no-table-part nudge with an empty `file`; the documents answer. The targeting of the named document lives in `commands::mixed_tier` and cannot be frozen without Tauri. | kept; step 08 adds the targeting |
| K | `baseline_case_k_...` | "Explain the formula of column total in calculs.xlsx" and the French equivalent: `Structural(HasFormulas { has_formulas: true })`, zero calls. **Wider than the plan said:** a writing question ("Write a formula that adds base and total in calculs.xlsx") steps door 3 aside, but `prepare_for_mixed` returns the same yes/no and `mixed_answer::answer` returns it without generating (empty prose, zero calls). | DEFECT, steps 05 and 07 |
| L | `baseline_case_l_...` | "Write a mail to Alpha about his unpaid invoices in factures.csv": writing intent, no distributive word; `locate_fill_source` and door 3 are `None`; door 4 leaves `TryModel` (unrecognised) and the writer is called once. | kept (step 08 must not capture it) |
| M | `baseline_case_m_...` | Tables only, "Summarise factures.csv" / "Resume factures.csv": `TryModel` with an unrecognised nudge; with a gateway answering `{"unsupported": true}` the resolved answer is that nudge stamped with `model_attempt`, after exactly **one** gateway call (the hidden interpreter). | step 05 for advisory words |

**Cases that cannot be frozen without Tauri.** `commands::sourced_answer` and the retrieval of `commands::mixed_tier` need an `AppHandle`. Not frozen: the choice of door in `sourced_answer` (doors 0 to 2 and the order between them), case I's targeting of a named document, case J (the button under a computed answer, `skip_deterministic`) and the final `AskAnswer` shape. They stay with the human tests of steps 06 and 08.

## Measured results

- Pre-check, before the first edit (one run per file, `--test-threads=1`): `mixed_selection_routing` 5 passed, `mixed_answer` 22 passed, `fill_plan` 5 passed, `publipostage` 10 passed. No failure.
- New file: `cargo test --test mixed_routing_baseline -- --test-threads=1`: 13 passed, about 4 s. The first run failed on two expectations that were guesses, both corrected from what the code does: the figure line (`Sheet agencies, column amount: sum = ...`: the sheet of a CSV is its stem and the column is named) and case G (a column named by one workbook decides, see above). No production code was touched to make a test pass.
- rustfmt: `tests/common/counting_gateway.rs` and `tests/mixed_routing_baseline.rs` clean (the latter formatted in isolation, since `rustfmt` follows `mod common;` into files that already have diffs and must not be reformatted); `src/mixed_answer.rs` stays at 10 `Diff in` blocks (before: 10); `tests/mixed_selection_routing.rs` goes from 5 to 4.
- Final pass, once, after the last edit of code: `cargo test -j 2 -- --test-threads=1` 87 test binaries, 2 017 tests passed, 0 failed. A first attempt without `-j 2` stopped on a rustc internal compiler error ("no resolution for an import", rlib files not found) during the parallel build, before any test ran; it is a build-environment failure, not a test failure, and the rerun with two build jobs was clean. `cargo clippy --tests`: no warning in a file of this step (the warnings listed belong to the existing library code). `pnpm run build` OK; `pnpm run test` 114 files, 2 851 tests passed (this includes the guard that rejects French words in `.rs` files, which covers the new golden tests).

## Decisions taken

- The shared chat fake lives in `tests/common/counting_gateway.rs`, not in `fake_gateway.rs`, which serves `/v1/embeddings` only and cannot answer a chat request (the plan and three prompts said otherwise; corrected).
- Case B is frozen through `mixed_answer::answer` with a recording gateway rather than through the private `table_evidence_text`: that is what "what the model receives" means.
- Case G has two tests: the plan's "none named, ask which" and the already-existing "implied by column" behaviour, which step 02's rule 3 must agree with.

## Corrections made to later prompts (plan section 3, "Align the next prompts")

- Plan, section 1.3 case K: door 4 gets the same yes/no; section 1.4 and section 3 (fixtures): the chat fake is `counting_gateway.rs`, `fake_gateway.rs` is the embeddings fake.
- Prompt 02: a fact about `implied_by_column` already existing in the engine (`pick_target`), with the test that freezes it.
- Prompt 03: names of the three golden tests and of the flipping baseline tests (B, C, and the `table_unavailable` line of H and L); the chat fake to use.
- Prompt 05: door 4 must also consult the advisory test (`prepare_for_mixed` returns `no_table_part()`), the new name of the K and M tests, the shared gateway.
- Prompt 06: the shared gateway. Prompt 07: test door 4 too in the routing check. Prompt 08: the names and content of the H and L tests and what flips.
- Prompts 02 and 03 were read in full; 05, 06, 07 and 08 only where they quote this step's files (a search for the baseline file, the gateway helpers and the case letters). Prompts 04 and 09 to 26 were searched the same way and quote nothing that changed; they were not read in full.
