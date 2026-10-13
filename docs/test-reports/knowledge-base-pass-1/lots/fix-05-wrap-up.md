# Fix 5 - Wrap-up: the Analyse button and the final human test

13 October 2026 · branch `fix/kb-wrap-up` · based on `kb-after-fix-04-batch-3` (`c85fae6`) · tab "KB fix 5 - Wrap-up (button and final test)"

Written for the developer or agent who follows, and for the owner. English; French sentences are quoted test data.

## Summary

KBD-47 is fixed on the client: the Documents card's « Analyser » button counts only the document kinds, so an `.odt`, a `.vtt`, a workbook dropped in the folder, an image with no text or an unreadable file no longer keep it lit with a number. The final human test was rewritten for the code as it now stands, with a Swiss part, a Mac part and a results book of one block per step id; every expected figure comes from one replay of the whole protocol (`print_the_figures_of_the_final_protocol`), whose folders were compared with the ones the real helper builds. One new defect (KBD-48) was found by that replay and recorded; nothing else was built.

## What changed

- `apps/desktop/src/lib/analysis.ts` - `countPendingDocuments`, `analysisPendingDocuments` (document kinds in `discovered` or `pending`; same rule as the Rust `unanalysed_documents`, so the button and the « non analysés » warning agree). `countPending` and `analysisPending` stay, used by the Data card, which does not change.
- `apps/desktop/src/components/WorkFolderCard.tsx` - uses the two new functions.
- `apps/desktop/src/lib/analysis.test.ts` - three cases, seen failing first (`countPendingDocuments is not a function`).
- `apps/desktop/src-tauri/tests/knowledge_validation_walk.rs` - the ignored test `print_the_figures_of_the_final_protocol` and its helpers (cards, names, targeting lines, table answers, router answers, the knowledge window, the restore walk).
- `human-tests/lot11_helper.py` - `extra-files` (the Swiss and French amount tables and the batch 3 files), a macOS process check (not run on a Mac), usage text; `human-tests/lot-11-files/data/` holds the two amount tables the helper and the replay both read.
- `human-tests/lot-11-validation.md` - rewritten: Parts 0, A, B, C, D, S (Switzerland, tables, file names, mixed tier, knowledge window), G (200), E, Z (optional), R, M (Mac, skippable).
- `02-results.md` - generated from the protocol's own `Expected:` text (133 blocks, summary by part).
- `defects-register.md` (KBD-47 fixed, KBD-48 added), `docs/DECISIONS.md`, `docs/SESSION-KB-STATUS.md` (local).

## Decisions taken

| Decision | Reason |
| --- | --- |
| Client-side count, not a count sent by Rust | The change is five lines; the Rust function already holds the same rule and its test |
| The "Analyse card" lines are the status line (`N fichiers, M analysés, K illisibles`) plus the pass lines | Read from the card code: the permanent line carries « analysés », the pass summary has no such line. The old protocol's « 1 analysé » was wrong |
| Swiss part placed at 100 documents after Part D, the knowledge-window deletion last in it | Their expectations depend on the library state; the deletion flags 92 documents to re-read, which Part G's Analyse then shows |
| Results book generated from the protocol | One source of truth: the two cannot disagree |

## Deviations

- The old protocol's figures changed with the batches: `Jean Dupontet` keeps 1 file (was 7), `Dr Martin` offers two doctors (was three people), `CHUV` and `Dre Vuagniaux` now narrow correctly, the shrink back to 30 documents returns exactly the counts before the growth (the KBD-12 promotion no longer shows).
- The helper writes the two letters of `lifecycle add` and `add-spaced` with Windows line breaks, the replay with `\n`: same 216 files and names, two sizes differ by the line breaks.

## Verification (Windows, this PC, once at the end)

| Command | Result |
| --- | --- |
| `cargo test --lib --tests --no-fail-fast` | **1 314 passed**, 0 failed, 43 ignored (batch 3: 1 314 / 42; the new ignored test is the one more) |
| `pnpm run test` | **630 passed** (627 + 3) |
| `pnpm run build` | OK |
| `cargo clippy --lib` | 21 warnings (baseline) |
| `rustfmt --edition 2021 --check tests/knowledge_validation_walk.rs` | clean |
| macOS | **not run** (Mac mini not delivered); Part M is written from the code and says so |

## Not done

- Anything outside the two tasks (owner's decision of 13 October). The new readers, F-17 to F-19 and F-21 stay out.
- The window was not seen by the agent: lines on screen, colours and buttons are what she checks. The text recognition of the scans is perfect in the replay and real on her machine.
- B6 (follow-up of the previous question) could not be replayed: it depends on the conversation; it states the known defect KBD-19.

## Documentation

`defects-register.md`, `docs/DECISIONS.md`; `SESSION-KB-STATUS.md` (local).

## Human test

`human-tests/lot-11-validation.md` with `lot11_helper.py` and `02-results.md` - proposed, not run. She plays it once and writes her report in `02-results.md`; the programme ends there.

## Open questions

- KBD-48: after a name is deleted and restored, the table line of `patients-ch.xlsx` (a lone surname in column `Nom`) does not come back. Cause unconfirmed, deferred.

## Next

Nothing is planned after her report except the fixes it calls for.

## Git

See the final message to the owner.
