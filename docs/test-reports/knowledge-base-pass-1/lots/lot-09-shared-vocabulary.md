# Lot 9 - Services on the shared vocabulary

12 October 2026 · branch `feat/kb-shared-vocabulary` · based on `kb/integration` at `51970e5` (tag `kb-after-lot-08`, lot 8 merged) · tab "KB 9 - Shared vocabulary"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

Five read-only services now read the names of the knowledge base for the features that need a vocabulary: ranked names for speech and autocomplete, proposals to repair a transcript, questions worth asking, the sheet of one name, and a guard that finds every known name in a text about to leave. None of them stores or learns anything, none reaches the AI or a log, and only the questions and the sheet have a face in the window: a block of **suggestions** in the empty conversation and at the bottom of the Documents card (each proved answerable on the user's own files by a dry run, names hidden until she asks), and a small **i** beside a name (on its chip, in the `@` list) that opens its sheet inside her selection. The walk of the human test on its own fixtures found four defects of the existing product (KBD-21 to KBD-24); the lot handles each one without fixing it.

## What changed

**Rust, new (`src-tauri/src/knowledge/`)**

- `surface.rs` - `surface_forms` (ranked names by purpose, selection first), `spoken_form`, `estimated_tokens`, and the loader `known_entities` the other services share.
- `repair.rs` - `propose`: capitalised or titled words and runs of up to three words read against the confirmed names of the selection; `Suggest` or `Ask`; never rewrites.
- `suggest.rs` - `suggest` (eight template kinds as machine data, dry runs through `tabular::engine::execute`, at most five), `SuggestionCache`, `cache_key`, `COLD_READ_LIMIT_BYTES`.
- `sheet.rs` - `entity_sheet` (the selection's files, counts only for the rest) and `files_for_entities`.
- `guard.rs` - `OutboundGuard::build`, `scan`, `strip`: every live name, global on purpose.

**Rust, existing files (additive)**

- `commands.rs` - `knowledge_suggest_questions`, `knowledge_entity_card`, `ResolvedSelection`/`resolve_selection`, a `SuggestionCache` in `AppState`. `lib.rs` - two lines. `knowledge/mod.rs` - five `pub mod`.
- `tabular_answer.rs` - `WorkbookChoice`, `selected_workbook_choices`, `OpenedWorkbook`, `open_single_sheet_workbook` (cold reads bounded). `index_store.rs` - `has_tabular_workbook`. `knowledge/query.rs` - `Word` and `words_of` made `pub(crate)`.

**Interface (`apps/desktop/src`)**

- `lib/suggestions.ts`, `lib/entityCard.ts` (pure: every sentence is written here from the catalogue), `state/useSuggestions.ts`, `components/SuggestionChips.tsx` (`SuggestionList` + the hook), `components/EntityCardDialog.tsx` (`EntityCardSheet` + the loader).
- Wired in `App.tsx` (the session reveal of names, the question requested from a card, the refresh key), `ChatPanel.tsx`, `MessageList.tsx` (the empty-state slot), `Composer.tsx`, `EntityChips.tsx`, `EntityPicker.tsx` (the **i** button), `WorkFolderCard.tsx`, `DataFolderCard.tsx` (the block under the summary), `lib/ipc.ts` (types, `suggestQuestions`, `entityCard`), `locales/{fr-FR,en-US}.json` (`suggestions.*`, `entityCard.*`), `styles.css`.

**Tests and tools**

- `tests/knowledge_surfaces.rs` (41 tests + 1 ignored benchmark), `tests/knowledge_surfaces_walk.rs` (an ignored printer), `tests/common/suggestions.rs`, `tests/fixtures/knowledge/repair-fr.json` (the contract file), one test added to `tests/knowledge_isolation.rs`; `lib/suggestions.test.ts` (15), `lib/entityCard.test.ts` (10), additions to `components/render.test.ts` and `lib/ipc.test.ts`.
- `human-tests/lot-09-shared-vocabulary.md` and `human-tests/lot09_helper.py`.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 9". The ones to know:

| Decision | Reason |
| --- | --- |
| The sheet's command is `knowledge_entity_card`, its component `EntityCardDialog`, not `knowledge_entity_sheet` / `EntitySheet.tsx` | Lot 8 already owns both names for the management dialog's sheet |
| The `@` picker stays `picker::search_entities`; `surface_forms` with `Autocomplete` is the same idea ranked by weight, with no caller yet | The picker also reports the names outside the selection (lot 7), is accepted and tested, and the lot 8 hand-over says it stays |
| Repair: no `None` action; no candidate gives no proposal. Several runs from one word: the strongest wins, the shortest on a tie | `Duran a` must be `Duran`; a `None` entry would be noise |
| Repair targets are confirmed names only (a manual name counts) | A guess must not make the next guess look like a certainty (same rule as the name list) |
| Suggestions carry the **cell** a name was read from, reduced to the words that point at that cell alone (`askable_words`) | Found by walking the human test: the tables read `Dr` as an equality on `Dr` when three doctors are written `Dr ...` (KBD-24) |
| No question about the documents with documents **and two workbooks** selected; `folder_count` only with no workbook selected; no document name that reads as a file name; no table with several reachable sheets | KBD-21, KBD-23, KBD-18, and a sentence that names no sheet |
| A workbook whose cells are not cached is read for a suggestion only up to 2 MB | Owner rule of 11 October 2026: a screen no one asked for a file by name must not start reading a large one; an Analyse empties the cache, a question fills it |
| Names hidden: the result says `namesHidden`; the reveal is a `useState` of `App`, session only, the setting is untouched | D10; a stored reveal would defeat the setting |
| The refresh key holds the finished passes, the mode, the names setting and the language | Found while preparing the human test: without the mode, switching the mode off left the old block on screen |
| The guard is global, built per request, and exposed to no command | The one exception to selection scoping; a command returning it would be a directory of the names the base holds |

## Deviations

- **From the lot file:** (1) `knowledge_search_entities` is not rebuilt over `surface` (see above). (2) The optional « also in these documents » row after an answer is not built. (3) The sheet is not opened from the management dialog (it keeps its own, richer sheet); it opens from the chip and the `@` list. (4) `knowledge_files_for_entities` is a Rust function (`sheet::files_for_entities`), not a command: nothing in the interface needs it. (5) The repair's `None` action is the absence of a proposal. (6) The `title_spoken` mapping already existed in the packs (lot 3); no pack was edited. (7) `suggest` also takes a bound on cold reads (`max_cold_read_bytes`), which the lot file does not mention.
- **From the master:** none found. "What the code looks like today" matches for the symbols this lot touches (`sourced_answer`, `tabular::engine::execute`, `question::example_columns`, `folder_questions::route_in_selection`).
- **Existing code changed beyond the lot's list:** `tabular_answer.rs`, `index_store.rs` (one method), `query.rs` (visibility), `Composer.tsx`/`EntityChips.tsx`/`EntityPicker.tsx` (one prop each), `render.test.ts` (the new props).

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a read changes the base | `no_service_of_the_lot_writes_a_row` (the snapshot of every `kb_` table), `the_vocabulary_services_change_no_row_and_are_the_same_every_time` (`tests/knowledge_isolation.rs`) |
| I4 - every failure falls back | A failed lookup is no suggestion (`useSuggestions`); the services return an error code and the question path does not call them |
| I7 - nothing outside the selection is revealed | `the_ranked_names_come_only_from_the_selection`, `a_name_known_only_from_other_files_is_not_revealed`, `a_repair_proposes_only_names_of_the_selection`, `suggestions_use_only_the_names_of_the_selection` (the serialised result is searched for outside names), `the_sheet_lists_the_files_of_the_selection_and_only_counts_the_rest` (the serialised sheet is searched for outside paths). **The one exception is the guard**, tested as such (`the_guard_is_global_and_ignores_every_selection`) and recorded in `PRIVACY-AND-SECURITY.md` |
| I8 - nothing the base holds reaches a model or a log | `the_new_modules_have_no_network_no_log_and_no_prompt` (a source scan of the five modules); no prompt was touched, the existing guards pass |
| I9 - model-facing strings stay neutral | None added |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the source guard reads the five new modules); the catalogue parity; `suggestions.test.ts` reads the template ids from `suggest.rs` and checks both catalogues |
| I11 - no destructive action on user files | Nothing writes a file; `lot09_helper.py verify-files` in the human test |
| I12 - the webview never talks to the server | Two Tauri commands; `ipc.test.ts` pins their names and arguments |
| I13 - no `cfg`, no Windows path literal | None added; the tests use temporary folders. **Not run on macOS** |
| I14 - idempotence | Untouched; the services write nothing |
| The sentences are proved | `a_mixed_selection_gets_every_kind_and_each_one_is_answered_in_both_languages` and the neighbouring tests fill every catalogue sentence, in both languages, and put it through the real folder router and the real tabular classifier (and the data-only step of the mixed tier) |

## Verification

Windows 11, debug profile. Baseline taken at the start of the session on `51970e5`: `cargo test` (all targets) finished with exit code 0, and the figures of the lot 8 report apply to the same commit.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps\desktop\src-tauri`) | 1202 passed, 0 failed, 19 ignored | **1244 passed, 0 failed, 21 ignored** (+42: 41 in the new `tests/knowledge_surfaces.rs`, 1 in `tests/knowledge_isolation.rs`; the two new ignored tests are the walk printer and the guard benchmark). Of which `--lib` (what CI runs): **748**, unchanged (this lot's tests are integration tests) |
| `cargo clippy --lib` | 21 warnings | **21** (none in the new files) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it), `tests/knowledge_surfaces.rs`, `tests/knowledge_surfaces_walk.rs`, `tests/common/suggestions.rs`, `tests/knowledge_isolation.rs`; `commands.rs` has 8 hunks (8 on `HEAD`), `tabular_answer.rs` 43 (43), `index_store.rs` 4 (4). No plain `cargo fmt` was run |
| `pnpm run test` (from `apps\desktop`) | 571 passed (37 files) | **622 passed** (39 files; +48: 15 + 10 in the two new files, +13 in `render.test.ts`, +2 in `ipc.test.ts`, +9 in the source guards that read the new components and modules) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

**What the agent ran that is not a suite:** `cargo test --test knowledge_surfaces_walk -- --ignored --nocapture` on the thirty-one documents and the two spreadsheets `lot09_helper.py write-fixtures` writes (the figures of the human test); every helper command of the protocol in order on a temporary sandbox (`ACAI_CONFIG_DIR`, `ACAI_DATA_DIR`, `ACAI_BACKUP_DIR`), including a `verify-files` that catches a changed byte; a **mutation check**: with `askable_words` replaced by the whole cell, `a_name_written_with_a_title_in_a_column_is_asked_without_the_title` fails.

**Not run:** the application window (nothing was clicked, no screenshot), a gateway turn of a suggested question, macOS, a release `tauri build`.

**An accident to report:** a `git stash` ran by mistake inside a formatting check; the working tree was restored at once with `git stash pop` (the stash list is empty, every file back). Nothing was lost.

## Measured

Windows 11, the owner's development PC.

- **The guard (release, `cargo test --release --test knowledge_surfaces -- --ignored --nocapture`):** building it over 10 000 names takes **10.5 ms**; a scan of 2 006 bytes against them takes a **median of 108 µs** (budget: 5 ms). A guard is built per request.
- **A suggestion refresh (debug profile, `cargo test --test knowledge_surfaces_walk -- --ignored --nocapture`):** 36 to 43 ms for the six selections of the human test, names hidden or shown, with up to four dry runs, **including** reading the folder inventory and opening the Data folder, which the window's command also does. A second look at the same screen is answered from memory. The window is never frozen by it (the command is `async`) and it starts only when a screen that draws it is shown and the selection is not empty.
- **Not measured:** the time a suggestion adds to the window when the workbook is large and cold (the bound of 2 MB exists so that it stays small).

## Not done

- The « also in these documents » row after an answer (optional in the lot file).
- Opening the sheet from the management dialog.
- A command for `files_for_entities`, and the thin `surface` command for the picker.
- Speech capture and engines (not this lot), the spoken-form text for titles other than the base pack's.
- Running the window; macOS. The English sentences were proved by the real classifier and router, never seen on screen.

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 9"), `docs/PRIVACY-AND-SECURITY.md` (three rows, the guard exception among them), `docs/CLIENT.md` (the questions to try and the sheet), `docs/ARCHITECTURE.md` (a paragraph on the services), `defects-register.md` (KBD-21 to KBD-24 and the notes), this report, the human test and its helper. Not touched: `docs/RETRIEVAL.md`, `docs/SELECTION-AND-MEMORY.md`, `docs/LANGUAGE-AND-LOCALE.md` (no new rule), `docs/KNOWLEDGE-BASE.md` (lot 11's).

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-09-shared-vocabulary.md` - **first run by the owner on 12 October 2026: Preparation, A to E and G matched, every clicked suggestion answered; three product defects and one wording change found, all fixed in the branch** (the small **i** invisible on the highlighted row and on the chip; **the block vanishing at the bottom of the Documents card after a second « Analyser »**, because it was drawn inside the analysis summary, which is empty after a pass that changed nothing and sits inside a disclosure; **the two folder cards of « Réglages » squashed**, because the sheet's style rules used the class name `card` that the folder cards use; title now « Suggestions », line « Questions suggérées à partir de vos fichiers », the promise dropped). Each is covered by a test where a test can see it (`render.test.ts`: the sheet's own class names, the new wording; `analysisScope.test.ts`: which card shows the block). **Re-run needed** (Preparation, A, B, E1, E2, F0, F1, S1, S3, H); the protocol says which steps. Original status line: proposed, not yet run. Visible in the application: **partly**. Visible: the questions to try (empty conversation and under the analysis summary), the button that shows the names, the sheet of a name from a chip and from the `@` list, the mode switch. Not visible: `surface`, `repair` and `guard` (Rust-only): the protocol points at the tests that prove them. Parts: Preparation, A (nothing selected), B (all documents, names hidden), C (names shown, three documents), D (a spreadsheet, then two), E (the sheet), F (the mode off and back), G (no file touched), smoke test, H (optional, names always shown), the only restore. About 55 minutes. Every displayed suggestion is clicked and must answer. **The agent did not open the application window.**

## Open questions

For the owner:

**Answered by the owner on 12 October 2026, all recorded in `docs/DECISIONS.md` ("Settled by KB lot 9", row "Owner answers") and in the register's notes of lot 9, as a watch list to come back to:**

1. **The promise « elle a une réponse »:** too strong. **Nothing is affirmed that cannot be proved**; the block says « Questions suggérées à partir de vos fichiers ». Stay prudent in what the interface says about what the product can answer; solutions to KBD-21 to KBD-24 are to be looked for later.
2. **The file's name in the sentence** (« ... dans rdv.csv ») stays, same prudence; revisit if it blocks (KBD-04: `ventes-2025.xlsx` puts `2025` in the sentence).
3. **The small « i »** stays on the chip and in the `@` list; removed later if it proves intrusive.
4. **A workbook above 2 MB with a cold cell cache** gives no suggestion until a question has read it: accepted for now, to be watched in use.

Defect rows: **KBD-21, KBD-22, KBD-23 and KBD-24** added (all `deferred`, none the knowledge base's doing); notes on KBD-04, KBD-11, KBD-13, KBD-12, KBD-14, KBD-15, KBD-16, KBD-17, KBD-18, KBD-19 and KBD-20. **No row is `blocking`.**

## Next

Lot 10 (`SESSION-KB-LOT-10-Activity-profiles.md`, tab "KB 10 - Activity profiles", branch `feat/kb-activity-profiles`) can start once this lot is merged and tagged `kb-after-lot-09`; **lot 11** (`SESSION-KB-LOT-11-Prove-it.md`, tab "KB 11 - Prove it") comes after lots 8, 9 and 10. What lot 10 and lot 11 must know from this lot:

- The services take `&QueryVocabulary` (packs, titles, encoder) and `Option<&QuerySelection>`; nothing in them depends on the conversation.
- `known_entities` (in `surface.rs`) is the shared loader: confirmed names are `!is_unconfirmed()`; a typed name counts even while a candidate.
- The suggestions' cache key includes `gazetteer_epoch`, the sizes of the mention, entity, source, log and cached-workbook tables: a new table that changes what a suggestion would say must join it.
- Lot 11: a human question that names a doctor with his title against a `Dr ...` column answers 0 (KBD-24); with documents and two workbooks a document question gets « Quel classeur ? » (KBD-21); the carry-over of the previous answer's person (KBD-19) meets the short, name-free suggestions in `auto`.
- Source guard reminders (unchanged): no word of le/la/les/des/une/est/erreur/fichier/dossier/envoi/veuillez/aucune in `src-tauri/src`; no `> text <` on one line of a `.tsx`.

## Git

```text
git status
git add apps/desktop/src-tauri/src apps/desktop/src-tauri/tests apps/desktop/src docs/ARCHITECTURE.md docs/CLIENT.md docs/DECISIONS.md docs/PRIVACY-AND-SECURITY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: add entity suggestions, transcript repair and an outbound guard"
git push -u origin feat/kb-shared-vocabulary
gh pr create --title "feat: add entity suggestions, transcript repair and an outbound guard" --body "<see the final message>" --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-09
git push origin kb-after-lot-09
git switch -c feat/kb-activity-profiles
```
