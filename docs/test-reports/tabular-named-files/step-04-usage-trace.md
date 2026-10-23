# TABULAR step 04 - What happened to each file (23 October 2026)

Branch `feat/workbook-usage-trace`, from `feat/tabular-integration` at the merge of step 03b (`d0632e2`). The choices are in `docs/DECISIONS.md`, "Taken while building TABULAR step 04" (rows 1 to 15).

Principle: **a file the user named never disappears without a visible line saying what was done with it.** Step 03 filled `AskAnswer.workbook_use` for the model path only and showed nothing; this step completes it for every door and renders it.

## What changed

- **`workbook_use.rs` (rewritten).** `WorkbookUseHow` = `card_sent | computed | plan_prepared | not_used`; `UseReason` (an enum, was a string) = the four `implied_*`, `named_not_selected`, `not_analysed`, `unreadable`, `changed_since_analysis`, `ambiguous_name`, `not_relevant`, `not_sent`. `WorkbookUse` gains `truncated`, `count`, `candidates`. **`trace(&SelectionFacts, Answered) -> Vec<WorkbookUse>`** is the one function every door ends with (`Answered::Mixed | Engine | Plan | FolderSearch`); `of_mixed_answer` is gone, its card-and-figure half is inside `trace`.
- **`tabular_answer::selection_facts`** (new): what the question names, the ticked workbooks that cannot be used (`Unusable::ChangedSinceAnalysis | NotAnalysed | Unreadable`, the dropped entries `chosen_workbooks` hides), the ticked usable ones, all from the original `scope.data_mode`.
- **`commands.rs`:** the doors with a table (`tabular_tier`, `mixed_data_only_tier`, `mixed_folder_tier`, `fill_tier`, `mixed_tier`) set `workbook_use` through `trace`; the four constructors of the documents tier keep the default (no table is ticked there).
- **Interface:** `src/lib/workbookUse.ts` (pure: entries to lines, which are cautions, folding), `src/lib/annotations.ts` (`annotationParts`, the workbook lines last; `cautionLines` adds the cautions; `not_asked_about` skipped once the trace holds an entry), `src/components/WorkbookUseNotes.tsx` (folds more than three lines, the cautions stay in view), `src/state/useChat.ts` (`ChatEntry.workbookUse`, merged once with `targeting` and `diagnostics`), `src/lib/ipc.ts`, keys `workbookUse.*` and the rewritten `chat.mixedTableUnavailable.not_asked_about` in both catalogues.

## Tests

- `workbook_use.rs`: 14 unit tests (trace per outcome, serialisation).
- `tests/workbook_use_trace.rs` (new, 12 tests, no socket): door 4 (named, implied by a word, none concerned, single ticked), door 3, tables only (named but not ticked), ambiguous name, door 1, door 2, a changed or vanished entry, a never-analysed entry, a changed entry that the question names.
- Vitest: `workbookUse.test.ts` (new), `annotations.test.ts`, `readingParts.test.ts`.
- Final pass (once, after the last edit of the main work): `cargo test -j 2 -- --test-threads=1` 2 135 passed, 0 failed; `cargo clippy --all-targets` exit 0 (warnings in files this step did not touch); `pnpm run build` and `pnpm run test` (2 879 tests) green. Two small corrections followed the walk on her real index (DECISIONS rows 13); after them `cargo test --lib` (1 268 passed), `--test workbook_use_trace` and `clippy` were run again, not the whole suite (the change is two additions read by `trace` only).
- Pre-check before the first edit: `workbook_use` and `tabular_answer::` unit tests, 64 passed.

## Found by the walk on her real index (copy of `index.sqlite3`, the Data folder read only)

Her Data folder holds `donnees_publipostage.csv` at the root and in `kb-lot05/` (and in `Test/publipostage/`). `Quel est le total du montant de donnees_publipostage.csv ?` with both ticked: the engine answers from the root file (exact path), `named_workbooks` calls the name ambiguous, so the first trace said "none was used" under an answer computed from one. Corrected in the trace (DECISIONS 13). **Not fixed (14):** door 4 still sends no card for that file; the two resolvers disagree (`resolve` step 1 against `whole_name_hits`). To decide in step 08.

## Corrections made to later prompts

- `SESSION-TABULAR-05`: `advisory_tier` must set `workbook_use` with `trace(.., Answered::Mixed { .. })` (or `Answered::Engine(&nudge)` for the fallback); the lines are in `workbookUse.ts`.
- `SESSION-TABULAR-06`: `of_mixed_answer` no longer exists; the `explained` entry needs a `WorkbookUseHow` variant, an `Answered` variant, the TypeScript union, a `case` in `lineOf` (a `switch` with no default) and the key `workbookUse.explained`.
- `SESSION-TABULAR-08`: the trace is `trace(.., Answered::Plan | Engine)`; `read_file` must learn the three new outcomes; `UseReason::SheetToChoose` does not exist yet (Rust, TypeScript union, catalogue key).
- Plan 4.3: `card_truncated` is a flag; the codes and the single function are written.

## Not done, by choice

- No trace on a question on documents alone (DECISIONS 11).
- Folder of a workbook of the other family is not named in the trace line (the Sources line has the same limit): the plain path only.
- Door 1 and door 2 are covered by the automatic tests, not by the human test (a template and a "documents" sentence would need a fixture choice the owner has not made).

## Owner's test (23 October 2026): results

Script: `docs/SESSION-TABULAR/reports/step-04-usage-trace-test-humain.md`. **Played on the development PC with the model served on localhost** (the tower `192.168.1.83` was off): same alias `qwen3:1.7b`, slower (32 to 54 s per answer). Nothing in the trace depends on the machine.

| Part | Result |
| --- | --- |
| A (selection) | Done. |
| B1 `Quel est le total du montant de factures-test.csv ?` | As written: 450, the two notes, `factures-test.csv : réponse calculée par le programme, sans l'IA.`, `2 tableurs cochés n'ont pas servi...`. |
| B2 `Résume factures-test.csv` | `cité dans votre question, l'IA en a reçu la structure.` + `2 tableurs cochés...`; neither old sentence; Sources `factures-test.csv (structure et 9 lignes sur 9, identifiants masqués)` not repeated by the trace line. |
| B3 `Quand reçoit le Dr Paul Martin ?` | Only `3 tableurs cochés n'ont pas servi...`; no old sentence. |
| B4 `reglements.csv` not ticked | `« reglements.csv » est cité dans votre question mais n'est pas coché...` + `3 tableurs cochés...`. |
| B5 copy | The copied text carries the same lines. |
| C1 `Que peux-tu me dire de ce tableur ?` | Three `implied_by_word` lines, no aggregate. |
| D2 ambiguous name | Both lines exactly as written (`donnees_publipostage` with the two paths, then `3 tableurs...`). |
| D3 five workbooks | Five `implied_by_word` lines in the copy. **The fold on screen was not reported** (the copy is never folded): unconfirmed. |
| E1 to E4, E6 | As written (size 108, `tail`, restore, listing without the copy). |
| E5 | **The line `changé depuis` was not observed.** See below. |

**E5.** The selection of part D was lost when the application was closed in E1 (the selection is not kept across a restart; the script should have said so), so the copy was the **only** ticked workbook. The explorer unticks a ticked file that is modified, by design (owner): the screenshot shows the copy as `non analysé`, the box disabled, `Dont non cochables : 1 fichier non analysé`. The question then ended in the banner « Les documents choisis pour cette conversation sont absents ou ont été modifiés. Choisissez-les à nouveau. » (`AppError::ScopeUnavailable`, the existing refusal when **every** ticked workbook changed: raised before any answer, so no trace). `changed_since_analysis` is therefore proved by the automatic tests only (`a_workbook_that_changed_or_vanished...`, `a_ticked_workbook_that_changed_is_not_also_reported_as_not_ticked...`).

### What was not investigated

| # | Finding | Status |
| --- | --- | --- |
| N-1 | In the screenshot the box of the modified copy is unticked but the counter still says `Utilisés : 0 doc. · 1 tab.`, and the ask was sent with that entry (hence `ScopeUnavailable`). If the explorer unticks a modified file on purpose, the counter and the scope entry may still hold it. | Not investigated; to look at when the explorer is next touched. |
| N-2 | With `qwen3:1.7b` the Sources lines of C1 and D3 say `structure seulement` (no rows sampled: the room is too small for several tables), while B2 (one card) sent 9 of 9 rows. | Step 03b's rule (the room is the model's); not a defect of this step. |
| N-3 | D3: the model describes four of the five workbooks it was sent (the `kb-lot05` copy is missing); B3: the answer repeats the question before the excerpt. | Model quality (1.7b); the trace says what was sent, which is true. |
| N-4 | The scripts that close the application must re-tick afterwards: the selection is not persisted. | Rule for the next human tests (memory note). |

Verdict: parts A to D and E1 to E4, E6 pass; the trace is correct for every door the test reached. The step can be merged; `changed_since_analysis` in the real window can be re-tried in a session with several workbooks ticked, without closing the application.
