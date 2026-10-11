# Lot 8 - Knowledge management dialog

12 October 2026 · branch `feat/kb-management-dialog` · based on `kb/integration` at `1027f01` (tag `kb-after-lot-07`, lot 7 merged) · tab "KB 8 - Manage knowledge"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

A full-window administrative dialog now shows everything the knowledge base holds, **with where each association comes from** (the file, the page or the cells, the method, the confidence, the passage around the name), and lets the user correct and extend it: create an entity, edit it, add a name, merge and unmerge, separate two people written with one name, delete a false positive and restore it, accept or dismiss a possible match, undo the last operation, reset the base (keeping or not what was typed). It opens from the settings and from a link under each analysis summary, never from the conversation. Every write is one transaction logged by ids; nothing touches a file; a change that adds or removes a name only makes the documents *due*, and they are read again from the stored text inside the next Analyse the user starts.

## What changed

**Rust**

- `src-tauri/src/knowledge/manage.rs` (+ `manage/tests.rs`) - new. The reads (`overview`, `list_entities`, `entity_detail`, `possible_matches`, `health_report`, `types_and_roles`, `last_operation`) and the writes (`create_entity`, `edit_entity`, `add_alias`, `remove_alias`, `set_attribute`, `remove_attribute`, `merge`, `unmerge`, `split`, `delete_entity`, `restore_entity`, `accept_possible_match`, `dismiss_possible_match`, `undo_last_operation`, `reset`). 36 unit tests.
- `knowledge/store.rs` - an additive column `kb_mentions.split_from` (added to a base from lots 1 to 7 by `ensure_schema`), `places_moved_away` and one skip in `apply_mentions` so that a re-reading does not undo a split, `log_op` made `pub(crate)` and returning the line's id.
- `error.rs` - four machine codes: `knowledge_name_invalid`, `knowledge_name_taken`, `knowledge_operation_refused`, `knowledge_nothing_to_undo`.
- `commands.rs`, `lib.rs` - 21 commands (`knowledge_overview`, `_list_entities`, `_entity_sheet`, `_possible_matches`, `_health`, `_types_and_roles`, `_create_entity`, `_edit_entity`, `_add_alias`, `_remove_alias`, `_set_attribute`, `_remove_attribute`, `_merge`, `_unmerge`, `_split`, `_delete_entity`, `_restore_entity`, `_accept_match`, `_dismiss_match`, `_undo`, `_reset`), all `#[tauri::command(async)]`, one additive block each.
- `knowledge/mod.rs` - `pub mod manage;`.
- `tests/knowledge_manage.rs` (10 tests, real Analyse + fake gateway) and `tests/knowledge_manage_walk.rs` (an ignored printer that walks the human test).

**Interface**

- `components/KnowledgeDialog.tsx` (shell, strip, tabs, error banner with retry, undo, confirmations) and `components/knowledge/{EntitiesTab,EntitySheet,EntitySheetParts,ReviewTab,TypesTab,ResetTab}.tsx`.
- `lib/knowledgeManage.ts` (everything that could be wrong, as pure functions: the list query, paging, locators in words, the undo button, the default survivor of a merge, `confirmationFor`, `isShortName`...), `lib/ipc.ts` (types and 21 wrappers).
- Entry points: `SettingsDialog.tsx` (**Gérer…**), `FolderCardParts.tsx` (`KnowledgeLink`) drawn by both folder cards, `App.tsx` (state and the dialog over whichever window is open).
- `locales/{fr-FR,en-US}.json` (`manage.*`, four `errors.*`, `settings.manageKnowledge*`, `analysis.knowledge.manageLink`), `styles.css`.
- `lib/knowledgeManage.test.ts` (45), `lib/ipc.manage.test.ts` (7), `components/knowledge/knowledge.render.test.ts` (21).

**Human check**: `human-tests/lot-08-management-dialog.md` and `lot08_helper.py`.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 8". The ones to know:

| Decision | Reason |
| --- | --- |
| No `knowledge_refresh` command: a name change raises `kb_meta.gazetteer_epoch`, the documents become due, and the existing end-of-Analyse refresh reads them | The owner's rule of 11 October 2026 (no costly silent background work); the lot file asked for a refresh "with progress", which an Analyse already shows |
| A merge across two types is **refused**, not confirmed | Lot 1 refuses it and the owner's rule says a module never transforms an entity type; the lot file said "confirm when the types differ" |
| A split needs a **new name different from every held name**; moved mentions become manual and carry `split_from`; `apply_delta` skips an automatic mention at a place moved away | Two entities with one name make the resolver ambiguous, which would drop the original's mentions at the next re-reading; without the skip, the next Analyse would undo every split |
| Restore turns the entity **manual** | An automatic entity with no mention is collected as an orphan, so a restored one would vanish before the next Analyse could find its documents |
| Editing an automatic entity turns it manual and keeps the old name as an alias | Documents that still write the old name must lead to the same entity, not create a second one |
| Removing a name, editing and setting a detail are **not undoable** | The log holds ids only (the lot file's rule); undoing them would need the text typed |
| Undo writes an `undo` line listing the lines it caused | So the next undo walks back one more operation and never toggles; tested |
| Identifier is not a type a person can create or edit to; a detail key may not be a personal identifier kind | Personal identifiers are stored as keyed hashes (lot 4); typing one in clear would bypass that |
| Commands are `async` | A busy database (an Analyse writing) can make one wait up to 3 seconds; it must not freeze the window |
| The dialog shows a short passage of the stored text next to each mention | The lot file asks for "page and passage"; on-screen only, never logged or sent, and the dialog is not reachable from the chat (recorded in `PRIVACY-AND-SECURITY.md`) |

## Deviations

- **From the lot file:** (1) no `knowledge_refresh` (see above). (2) Merge across types refused instead of confirmed. (3) **Hiding an automatic alias is not done** ("automatic aliases can be hidden, not edited"): every reader of aliases (resolver, gazetteer, prefix search, query detector) would have to honour a flag; the remedies are to delete or split the entity. (4) The "Library health" report lists counts and the first 50 items (entities seen once, documents with no entity, workbooks with no usable column, possible matches, people in documents and in no table), the wording is the interface's. (5) `remove_alias` and edits are not undoable (see above).
- **From the master:** `unmerge` and `delete` were already in the store (lot 1); the dialog only wraps them. The master's `knowledge_search_entities` was already delivered by lot 7. No discrepancy found between "What the code looks like today" and the code for the symbols this lot touches.
- **Existing code changed beyond the lot's list:** `store.rs` (column, skip, `log_op`), `FolderCardParts.tsx`, both folder cards and `SettingsDialog.tsx` (one prop and one block each).

## Invariants

| Invariant | Proof |
| --- | --- |
| I3 - nothing a question produces reaches the store | The new code has no caller in the question path; the lot 6 and 7 isolation tests pass unchanged |
| I4 - every failure falls back | A busy database gives `knowledge_unavailable` and changes nothing (`a_busy_database_gives_the_clean_code_and_changes_nothing`); the dialog offers a retry; the question path does not call this module |
| I6 - same relative path in two folders stays two sources | Untouched; the sheet groups mentions by `(domain, path)` |
| I8 - nothing the base holds is logged | The log holds a kind and ids (`kb_ops_log`); no `println`/log line was added to the product |
| I9 - no model-facing string | None added; the existing guards pass |
| I10 - machine codes only | `pnpm run test` (the source guard reads `manage.rs`, `manage/tests.rs` and every `.tsx`); catalogue parity; `every machine code the base can send has a label` (reads `Method`, `AliasKind`, `EntityStatus`, `Origin`, `PossibleReason`, the column reasons and the packs' predicates and subtypes from the Rust sources and the resource files) |
| I11 - no destructive action on user files | `no_operation_touches_a_document_a_chunk_or_a_table_row` (unit, byte-level rows of `documents`, `chunks`, `chunks_fts`, `tabular_*`), `nothing_the_dialog_does_touches_a_file_a_chunk_or_asks_the_ai_anything` (the files of the folder byte for byte, the index rows, no request to the AI), and `lot08_helper.py verify-files` in the human test |
| I12 - the webview never talks to the server | Every operation is a Tauri command; `lib/ipc.manage.test.ts` pins the names and arguments |
| I13 - no `cfg`, no Windows path literal | None added. **Not run on macOS** |
| I14 - idempotence | `a_manual_alias_is_found_in_the_next_question_and_in_the_next_scan` and `a_merge_survives_the_next_analyse...` (a second Analyse changes no row) |
| Chain of the operations | Every unit test ends with `integrity_check` clean; undo, merge chains, cycles, split and undo, delete then re-analyse, restore, resets (`manage/tests.rs`, `tests/knowledge_manage.rs`) |

## Verification

Windows 11, debug profile. Baseline measured at the start of the session on `1027f01`.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps\desktop\src-tauri`) | 1155 passed, 0 failed, 18 ignored | **1201 passed, 0 failed, 19 ignored** (+46: 36 in `knowledge::manage`, 10 in `tests/knowledge_manage.rs`; the new ignored one is the walk printer). `--lib` (what CI runs): **748** (712 before) |
| `cargo clippy --lib` | 21 warnings | **21** (none added) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it, `manage.rs` and `store.rs` included), `error.rs`, `tests/knowledge_manage.rs`, `tests/knowledge_manage_walk.rs`; `commands.rs` has 8 hunks, 8 on `HEAD`. No plain `cargo fmt` was run |
| `pnpm run test` (from `apps\desktop`) | 482 passed (34 files) | **571 passed** (37 files; +89: 45 + 7 + 21 in the three new files, +16 in the catalogue and source guards which read the new strings and components) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

**What the agent ran that is not a suite:** `cargo test --test knowledge_manage_walk -- --ignored --nocapture` on the thirty-one documents `lot08_helper.py write-fixtures` writes (whose figures are the human test's); every helper command of the protocol in order on a temporary sandbox (`ACAI_CONFIG_DIR`, `ACAI_DATA_DIR`, `ACAI_BACKUP_DIR`), including a `verify-files` that catches a changed byte.

**Not run:** the application window (nothing was clicked, no screenshot was taken, the dialog was never drawn by a browser: only `renderToStaticMarkup` of the sheet's sections), a gateway turn, macOS, a release `tauri build`.

## Measured

Nothing timed. The reads are indexed queries over a handful of rows (a page of 50, a sheet of at most 200 mentions each with one chunk read); the writes are one small transaction. `PackSet::load_with_type_vocabulary` is loaded per write command (about 40 ms in debug, an administrator's click), never per question.

## Not done

- Hiding an automatic alias (see Deviations).
- A refresh command with progress (replaced by the Analyse that already has one).
- Undo of removing a name, of editing and of setting a detail.
- The dialog was not looked at on screen by anyone: the owner's test is the first sight. Layout (two panes, tabs), keyboard order and the Escape key were not specified and are not handled beyond the confirmation window's own.
- Export/import of manual knowledge between workstations (master section 6, rule 9: a possible later feature).

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 8"), `docs/CLIENT.md` (the dialog), `docs/PRIVACY-AND-SECURITY.md` (the Reset and the operations rows, and the fact that the dialog shows names and a passage), `defects-register.md` (KBD-20 and the notes), this report, the human test and its helper. Not touched: `docs/ARCHITECTURE.md`, `docs/RETRIEVAL.md`, `docs/LANGUAGE-AND-LOCALE.md` (no new rule), `docs/KNOWLEDGE-BASE.md` (the long-lived spec is lot 11's).

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-08-management-dialog.md` - **proposed, not yet run**. Visible in the application: yes, fully. Parts: Preparation, A (open, find, read the provenance), B (add a name, the next Analyse uses it), C (merge and unmerge), D (separate two people, undo), E (delete a false positive, it does not come back, restore), F (files untouched), G (reset keeping what was typed), H (reset everything), smoke test, the only restore. About 55 minutes. **The agent did not open the application window**: the figures come from the walk through the same Rust code; every label was read from the catalogue.

## Open questions

For the owner:

1. **Merge across types is refused** (person with organisation, for example). Right, or should the dialog be able to change an entity's type first (it can: *Modifier*) and then merge?
2. **Restoring an entity makes it manual.** It then survives the "keep what I typed" reset. Acceptable, or should a restored automatic entity stay automatic and be allowed to vanish if no document names it?
3. **A name under three letters** (« JD ») is accepted and found in documents but never read in a question (lot 6 rule). The dialog warns while it is typed. Keep?
4. **Hiding an automatic alias** was not built. A wrong automatic alias can only be dealt with by deleting or splitting its entity. Is that enough for the pilot?
5. The sheet says « nom déjà connu de la base » for most mentions (KBD-13). Should the fix move forward before lot 11?

Defect rows: **KBD-20** added (deferred); notes on KBD-11, KBD-12, KBD-13, KBD-14, KBD-15, KBD-16, KBD-17, KBD-19. **No row is `blocking`.**

## Next

Lots 9 and 10 can start in any order once this lot is merged and tagged `kb-after-lot-08`; lot 11 comes after them. Recommended: **lot 9** (`SESSION-KB-LOT-09-Shared-vocabulary.md`, tab "KB 9 — Shared vocabulary", branch `feat/kb-shared-vocabulary`). What they must know from this lot:

- `knowledge::manage` is the read/write API of the dialog; lot 9's `surface.rs` can sit beside it. `manage::list_entities` is a management list (live, deleted, merged, with counts), not an autocomplete: `picker::search_entities` and `store::prefix_search` stay the autocomplete.
- A manual alias, a manual entity and a restored entity are `origin = manual`; `gazetteer::name_list_rows` already counts them (even while a candidate). A split entity has only its own name as an alias.
- `kb_meta.gazetteer_epoch` is raised by every management write that changes a name; `manage::documents_due` is the count of documents the next Analyse reads again.
- `kb_mentions.split_from` is the only schema change; the `kb_effective_mentions` view does not expose it (queries read `kb_mentions` directly).
- Source guard reminder: no word of le/la/les/des/une/est/erreur/fichier/dossier/envoi/veuillez/aucune in `src-tauri/src`, comments and test strings included; and no `> text <` on one line of a `.tsx` (a signature such as `=> Promise<...>` trips it: type it in a `.ts` file).
- The `lot08_helper.py` fixtures (31 documents with `note-jeannot.txt`) can be reused.

## Git

```text
git status
git add apps/desktop/src-tauri/src apps/desktop/src-tauri/tests apps/desktop/src docs/CLIENT.md docs/DECISIONS.md docs/PRIVACY-AND-SECURITY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: add a knowledge management dialog with provenance"
git push -u origin feat/kb-management-dialog
gh pr create --title "feat: add a knowledge management dialog with provenance" --body "<see the final message>" --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-08
git push origin kb-after-lot-08
git switch -c feat/kb-shared-vocabulary
```
