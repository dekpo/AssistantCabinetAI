# Fix 4, batch 3 - Table, router and check defects (F-12, F-13, F-15, F-22, F-23)

12 October 2026 · branch `fix/kb-retrieval-rules` · based on `kb-after-fix-04-batch-2` (`15088d5`) · tab "KB fix 4 - Batch 3"

Written for the developer or agent of the next batch, and for the owner. English; French sentences are quoted test data.

## Summary

The owner limited this batch to five fixes: the setting stays « Suggestions seulement », so F-17, F-18 and F-19 (the follow-up rule, the widening rule, the gate's own measure) are **not** done. Each of the five is proved by a test written first and seen failing (`tests/knowledge_fix_retrieval.rs`, 18 tests and one ignored print). A doctor written with his title is counted like the doctor without it; a month or year asked of an unsettled date column is a nudge naming the column instead of `0`; an export with a title line above its header is a table; a plain question about the documents is no longer blocked behind « Quel classeur ? » and « combien de documents » is no longer a row count; `mars/neurologie.pdf` names a file; a figure split by a line break draws no false warning. None of them touches what the base reads, so the gate figures of the sandbox cannot move and were **not re-measured** (no real-stack run).

## What changed

**Rust, `apps/desktop/src-tauri/src/`**

- `number_check.rs` - F-22: `scan_source` reads each source as written and once with line breaks taken for spaces.
- `file_reference.rs` - F-23: `paths_ending_with`; new reason `folder_suffix`; a reference with a separator that matches no whole path matches the path that ends with it at a folder boundary (one: exact, several: asked, none: not found).
- `tabular/csv_adapter.rs` - F-15: `consistency_score` counts only the lines a delimiter cuts into two fields or more.
- `tabular_answer.rs` - F-12: `settle_shared_words` (a word shared by several values of a column), `calendar_columns` (the single `ambiguous_date` column is the calendar when there is no `Date` column; its cell digits are no words), the nudge `non_date_column` carries the column in `filter_column`. F-13: `prepare_if_data_only` leaves a question that names the documents (unless a workbook is named) and a question that points at no table (`points_at_a_table`) to the mixed tier.
- `folder_questions.rs` - F-13: `names_the_documents`, `answer_in_mixed_selection` (count, list, tree about the selected documents; never a question that names a file).
- `commands.rs` - F-13: `mixed_folder_tier`, before `mixed_data_only_tier`.

**Client** - `apps/desktop/src/lib/tabularAnswer.ts`: the nudge for `non_date_column` also prints « Colonne concernée : {column}. » (existing catalogue line).

**Tests** - `tests/knowledge_fix_retrieval.rs` (new).

**Documentation** - `docs/DECISIONS.md` ("Settled by KB fix 4, batch 3"), `docs/WORK-FOLDER-INVENTORY.md`, `defects-register.md` (7 rows fixed, notes), `05-branches-and-stages.md`.

**Human test** - `human-tests/fix-04-batch-3-table-router-and-check-defects.md`, the helper `human-tests/fix04_batch3_helper.py` and the invented files in `human-tests/fix-04-batch-3-files/`.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| A word shared by several values of a column with nothing else picked is an `In` over the values that hold it (`Martin` alone: both Martins) | The battery allowed "do not filter, or `In`". Not filtering counted the whole table; the old `Equals` counted 0. `In` is what the sentence says and the « Compris comme » line shows it | DECISIONS |
| A single written date on an unsettled date column is the same nudge as a month or a year | It was dropped without a word and the whole table counted | DECISIONS, register notes |
| "Which workbook?" only when the classifier reads an operation or a column; `documents`/`files` words leave the data-only step unless a workbook is named | The battery's rule, with the folder router's own vocabulary (no second list) | DECISIONS |
| The mixed tier answers inventory questions about the documents itself, but never a question naming a file | The name can be a workbook of the Data Folder, which the document inventory does not know | DECISIONS |
| F-22 reads the **sources** with line breaks as spaces, the answer unchanged | A check can only gain a supported figure; nothing the model wrote is reinterpreted | DECISIONS |

## Deviations

- The battery's "Dr Jean Dupont" proof ("a column of three doctors written `Dr ...`") is run on a six-row file of invented doctors; the sandbox has none.
- "Seen failing first": every proving test failed on the code of the previous batch except the compile-only failure of `answer_in_mixed_selection` (the function did not exist) and the four "still holds" controls (a settled date column, the `Combien de lignes ?` choice, a comma file, a partial folder name), which pass before and after on purpose.
- Trade-off recorded: `Combien de lignes dans tous les fichiers ?` (plural `fichiers`, no workbook named) now goes to the mixed tier instead of the data-only step.

## Invariants

I1 to I14 unchanged: nothing the base reads, stores or narrows is touched (`knowledge_isolation`, `knowledge_targeting`, `knowledge_validation` green in the full run). I5 (the base never alters `tabular::engine` results): the engine is untouched; only what `detect_filters` builds for it changed. No model-facing string added (guard tests untouched). Rust holds no French literal (the language guard caught one comment, fixed).

## Verification (Windows, this PC)

| Command | Result | Before the batch |
| --- | --- | --- |
| `cargo test --lib --tests --no-fail-fast` | **1 314 passed**, 0 failed, 42 ignored (run twice: the second after the owner's replay, below) | 1 294 passed |
| `pnpm run test` | **627 passed** | 627 |
| `pnpm run build` | OK | OK |
| `cargo clippy --lib` | 21 warnings | 21 |
| `rustfmt --edition 2021 --check` | the new test file is clean; the touched source files keep findings that sit on lines of `HEAD` (never plain `cargo fmt`) | |
| macOS | **not run** (Mac mini not delivered); no `cfg`, no path literal, no new crate | |

**Owner's replay (12 Oct), step D3 failed:** the mixed tier itself still returned « Plusieurs tableurs sont sélectionnés » (also through « Demander à l'IA »). My walk had only tested the data-only step, not `prepare_for_mixed`. Fixed (`prepare_for_mixed` gives a non-table question no table part; `mixed_answer::resolve_table` reads it as "documents alone"), two tests added in `tests/mixed_answer.rs`, full run repeated once. Parts B and C of her replay matched the protocol.

## Measured

Not measured: the batch does not change extraction, resolution, targeting or retrieval, so the sandbox quality tiers and the real stack could not move. Baseline kept from batch 2: 93.3 / 97.0 / 96.5 % at 30 / 100 / 200 (the 98 % gate is abandoned by the owner's decision of 12 October).

Walked on temporary copies with the real helper commands (`ACAI_*` overrides) and the real code (`cargo test --test knowledge_fix_retrieval print_the_answers_of_the_human_test -- --ignored --nocapture`, `ACAI_WALK_DOCS`, `ACAI_WALK_DATA`): every expected value of the human test is that output.

## Not done

- F-17, F-18, F-19 (KBD-17, 19, 34, 37): out of this batch by the owner's instruction; they serve `auto`.
- F-22 is not visible in the window (it depends on the model copying a figure): tests only.
- The command wiring of `mixed_folder_tier` (Tauri) is checked by compilation and by the unit proofs of `answer_in_mixed_selection`; the window was not seen.

## Documentation

`docs/DECISIONS.md`, `docs/WORK-FOLDER-INVENTORY.md`, `defects-register.md`, `05-branches-and-stages.md`.

## Human test

`human-tests/fix-04-batch-3-table-router-and-check-defects.md` - proposed, not run.

## Open questions

- A word shared by several doctors and asked alone (`pour Martin`) now counts both. Should it rather ask which one? (A new "which value" answer: not done.)
- The same family remains for any column whose values share a word (suppliers `Alpha Services` / `Alpha Fournitures`): they are now an `In`.

## Next

Batch 4 is decided by the owner (F-20 the extractors for ODT, RTF, EML, VTT, ODP, PPTX, ODS; F-21), or the big human test. It must know: `CURRENT_KB_VERSION` is 3 (unchanged); `detect_filters` ends with `settle_shared_words`; the nudge `non_date_column` carries a column.

## Git

See the final message to the owner.
