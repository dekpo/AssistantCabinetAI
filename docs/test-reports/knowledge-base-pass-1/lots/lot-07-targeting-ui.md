# Lot 7 - Targeting in the interface

11 October 2026 · branch `feat/kb-targeting-ui` · based on `kb/integration` at `c9dc8bc` (tag `kb-after-lot-06`, lot 6 merged) · tab "KB 7 - Targeting in the interface"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The reduction the knowledge base makes is now visible and reversible in the window. Under an answer whose search was narrowed, a line says how many files were searched out of how many and for which name, with **Élargir à ma sélection** (the question is asked again on everything selected) and **Voir les fichiers**; a name that fits several people never blocks an answer but is asked about below it; in the box, `@` and the first letters of a name list the names of the selection and add a chip that narrows the next search; a name found only in files the user did not tick is offered apart, labelled, with a button whose click is the user's own. The settings window has a « Connaissances » group with the mode and the five thresholds, and a developer panel shows the timings and the recorded questions. With the mode `off`, or in `suggest` when no name needs a question, the screen is the old one.

## What changed

**Rust (small, additive; the lot file listed none, see Deviations)**

- `knowledge/picker.rs` (new) - `search_entities`: the names of the current selection that begin with typed letters (the store's prefix search), `active` only, each with its documents and workbooks of the selection; the names that are only in analysed files outside the selection, as paths, apart. An empty prefix, an empty selection find nothing. 0 `unsafe`, no write.
- `commands.rs` - `knowledge_search_entities(prefix, scope, limit)`, registered in `lib.rs`. It resolves documents and workbooks exactly as `sourced_answer` and `tabular_tier` do (inventory with the application's hash cache, `scope.resolve`, usable workbooks).
- `knowledge/query.rs` - `QueryAmbiguity.span` and `QueryPossible.span` (the words of the question as typed, `J. Dupont`), `EntityBrief.documents/workbooks` (one query for all the offered names, `count_offered`).
- `knowledge/targeting.rs` - `ReportCandidate.documents/workbooks`, `ReportAmbiguity.span`, `ReportPossible.span`.
- `settings.rs` - `knowledge_hide_names_in_suggestions` (default true, `serde(default)`), read by lot 9.
- `knowledge/mod.rs`, `lib.rs` - one `pub mod`, one command.

**Interface (`apps/desktop/src`)**

- `lib/targeting.ts` (new) - everything the user reads about a report, as pure functions: `targetingNote` (the line, the files), `targetingOffers` (which one? / did you mean?), `targetingRequestFor` (what is sent with a question), `carryOverIds`, `isShortFollowUp`, `confirmedNames`, `reductionApplied`.
- `lib/entityPicker.ts` (new) - `mentionAt`, `insertMention`, `insertAt`, chips, `pickerKey` (arrows, Enter, Tab, Escape), `debounce`.
- `lib/knowledgeSettings.ts` (new) - the limits of the five thresholds and the typed-number helpers; `lib/diagnostics.ts` (new) - the rows of the developer panel; `lib/analysisScope.ts` - `addScopeFiles` (shares `scopeOfEntries` with `toggleScopeFile`) and `combinedHasSelection`; `lib/annotations.ts` - the line is part of *Copier la conversation*; `lib/ipc.ts` - types, `searchEntities`, the `onTargeting` callback of `askWithSources`, the settings fields.
- `components/TargetingNote.tsx`, `EntityChips.tsx`, `EntityPicker.tsx`, `DiagnosticsPanel.tsx`, `NumberSetting.tsx` (new); `Composer.tsx` (the `@` list, the `@` button, chips, keyboard), `ChatPanel.tsx` (chips, retarget, props), `MessageList.tsx` (the note, the panel), `SettingsDialog.tsx` (group « Connaissances », « Avancé »), `App.tsx` (`addOutsideToSelection`), `state/useChat.ts` (`targeting`, `targetingChoice`, `diagnostics` on an entry, `retarget`, carry-over), `state/useEntitySearch.ts` (new; debounced lookup, stale answers dropped), `styles.css`, `locales/{fr-FR,en-US}.json` (`chat.targeting.*`, `chat.chips.*`, `chat.picker.*`, `settings.*`, `diagnostics.*`; five existing keys reworded).

**Tests and tools**

- `tests/knowledge_picker.rs` (16 tests + 2 ignored printers), `src/lib/{targeting,entityPicker,knowledgeSettings,diagnostics,ipc}.test.ts` and `src/components/render.test.ts` (new), additions to `analysisScope.test.ts`, `history.test.ts`, `annotations.test.ts`.
- `human-tests/lot07_helper.py`, `human-tests/lot-07-targeting-ui.md`.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 7". The ones to know:

| Decision | Reason |
| --- | --- |
| "Narrowed" is read from the **paths** of the report, never from `strategy` | `strategy` also says `entity_files` for a narrowing that was only measured |
| Widen is a **re-ask** (`useChat.retarget`, `targeting.mode = off`), and the choice stays on the answer | The lot file asked for "replacing the answer exactly as *Régénérer* does" and for *Régénérer* to keep the choice |
| The words of a "which one?" row come from the report (`span`), not guessed in the client | The first idea (the word the candidates share) fails on `J. Dupont`, titles and accents; Rust knows the span it settled |
| A "which one?" group is dropped whole once the user picked one of its names | Found by the agent's own walk: after picking Paul Martin the row kept asking about Claire and Pierre |
| A candidate with no file in the selection is dropped by the client even if Rust sent it | The second line of defence for I7; Rust now sends the counts that make it possible |
| Candidates are never shown as a name, never carried, never in the picker | Defect KBD-11, which asked lots 7 and 9 to do it. Exception, on purpose: the "which one?" row lists the unconfirmed names that fit the user's **own** words (documented by a test, KBD-11 note) |
| The picker returns `outsideSelection` for every name that has no file in the selection but files among the analysed ones | The lot file; at most 3 names and 50 paths a side, always labelled |
| Carry-over is sent only for a short question (8 words or fewer, the same constant as Rust, checked against `conversation.rs`) | The lot file: "sent only for a short follow-up". Rust still decides whether the question names nobody |
| The `@` button inserts an `@` (with a space before it after a word) | The lot file asked for "an always-available button"; reusing the `@` flow means one code path |
| Settings: the existing `knowledge_mode` select is relabelled **Ciblage intelligent** with the order Désactivé / Suggestions seulement / Automatique; shares are typed as percentages | The lot file; the pre-existing select already was the mode |
| A rendering test without a browser | The client has no DOM test library; `react-dom/server` renders each component to markup with the real catalogues (`render.test.ts`) |

## Deviations

- **From the lot file:** (1) **Rust was touched.** The lot file lists no Rust, but §7.3 says "`knowledge_search_entities`" as if it existed (it is only named in the master's section 9). It did not, and neither did `knowledge_hide_names_in_suggestions` (the settings section) nor the file counts and spans a defensive, correct "which one?" row needs. (2) **Voir les connaissances** (§7.4) is not drawn: the dialog is lot 8, and the lot file says "hidden until then". The knowledge lines of the analysis summary were already drawn by both folder cards (lots 4 and 5): nothing to add. (3) The per-workbook **hints** that lot 6 puts on « which workbook? » are not drawn: the order is Rust's, the annotation was not asked for in §7.
- **From the master:** none found. "What the code looks like today" matches for the symbols this lot touches (`ask_with_sources`, `AskAnswer`, `ChatStreamEvent`, `sourced_answer`, `tabular_tier`, `WorkFolderInventory::discover_with_cache`).
- **Existing code changed beyond the lot's list:** `analysisScope.ts` (a shared tail extracted from `toggleScopeFile`, behaviour unchanged, its tests untouched), `annotations.ts`, `ipc.ts`, five catalogue values (`settings.knowledgeModeLabel`, `knowledgeModeDescription`, `knowledgeMode.{off,suggest,auto}`, `showDiagnosticsLabel`, `showDiagnosticsDescription`).

## Invariants

| Invariant | Proof |
| --- | --- |
| I1 - candidates are a subset of the selection | Untouched; lot 6's tests pass |
| I3 - nothing a question says is learned | `searching_names_writes_nothing_to_the_base` (the picker); lot 6's `knowledge_isolation.rs` passes; a chip and the widen action are ids and a code, no row |
| I4 - every failure falls back | The picker's lookup failing is an empty list (`useEntitySearch`); lot 6's tests pass |
| I7 - nothing outside the selection is revealed | `ambiguity_chips_only_offer_entities_present_in_the_selection` (Rust, lot 6), `never renders a name that is in none of the selected files (invariant I7)` and the rendered `Marc Martin` check (client), `an_empty_selection_finds_nothing_even_when_the_base_holds_the_name`, `the_outside_group_never_lists_a_selected_file`, `a_file_changed_since_its_analysis_is_not_counted`; the outside group is the one labelled exception |
| I8 - nothing the base holds reaches a model or a log | Nothing model-facing was added. The question the model reads is the text the user wrote; a chip is an id (`ipc.test.ts`); the history carries the words only (`history.test.ts`) |
| I9 - model-facing strings stay neutral | No model-facing string added; the existing guards pass unchanged |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the source guard reads `picker.rs` and every file touched); catalogue parity for every new key; Rust sends codes and ids |
| I12 - the webview never calls the server | Every operation is a Tauri command (`knowledge_search_entities`, `ask_with_sources`, `knowledge_diagnostics`) |
| I13 - no `cfg`, no Windows path literal | No `cfg`; the tests use temporary folders. **Not run on macOS** |

## Verification

Windows 11, debug profile. The baseline is the run of this session before the first edit (the same figures as the lot 6 report).

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps\desktop\src-tauri`) | 1138 passed, 0 failed, 16 ignored (32 targets) | **1155 passed, 0 failed, 18 ignored** (33 targets; +17: 16 in the new `tests/knowledge_picker.rs`, 1 in `settings.rs`; the two new ignored tests are the printers). Of which `--lib` (what CI runs): **712** (711 before) |
| `cargo clippy --lib` | 21 warnings | **21** (none added) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it, `picker.rs` included), `settings.rs`, `tests/knowledge_picker.rs`; `commands.rs` has 8 hunks, 8 on `HEAD`; no plain `cargo fmt` was run |
| `pnpm run test` (from `apps\desktop`) | 365 passed (28 files) | **482 passed** (34 files; +117: 89 in the six new files - targeting 30, entity picker 22, settings limits 7, diagnostics 10, ipc 6, render 14 - then scope +13, history +1, annotations +3, and +11 in the source guard, which reads the new components and the new Rust module) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

**What the agent ran that is not a suite:** the two ignored printers (`print_what_the_interface_shows_for_the_human_test`, `print_the_workbook_choice_for_the_human_test`) on the thirty invented documents, whose figures are the human test's; the real sentences of both languages for each step (a throwaway vitest over the printed reports); every helper command of the protocol in order on a temporary sandbox (`ACAI_CONFIG_DIR`, `ACAI_DATA_DIR`, `ACAI_BACKUP_DIR`), with the state each part leaves checked against the next part's start.

**Not run:** the application window (nothing was clicked, no screenshot was taken), a gateway turn of `ask_with_sources` (the wiring was compiled and read; every function it calls is tested), macOS, a release `tauri build`.

## Measured

Nothing new is measured by this lot, and no timing was added to a question: the `@` list is one read of the folder (with the application's hash cache) and four indexed queries, started by typing and debounced to 120 ms; the line, the offers and the panel are computed from a report already in memory. The real figure to look at is the owner's, in step D4 of the human test (the panel's « Sélection » and timings).

## Not done

- **Voir les connaissances** (lot 8), the hints drawn on « which workbook? », question suggestions (lot 9).
- A relative widening rule (KBD-17) and a carry-over rule that tells a follow-up from a new short question (KBD-19): lot 11.
- Running the window; macOS; the keyboard handling of the picker on a Mac webview (the lot file asked to confirm it if a Mac is available: none).
- Screenshots of each state: the owner takes them while running the protocol (the results table has the column).

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 7"), `docs/CLIENT.md` (the four additions to the window), `docs/SELECTION-AND-MEMORY.md` ("The interface for it"), `docs/PRIVACY-AND-SECURITY.md` (the row for the `@` list), `defects-register.md` (KBD-19 and the notes), this report, the human test and its helper. Not touched: `docs/LANGUAGE-AND-LOCALE.md` (no new rule), `docs/RETRIEVAL.md`.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-07-targeting-ui.md` - **first run by the owner on 11 October 2026, partly received** (her message was cut at B6). Matched: Preparation, A1 to A7, B1 to B5, and on screenshots C1, C2, C3, C5, C6. Protocol wrong: her Data folder was analysed and holds names, so the `@` list offered extra outside-selection lines and « Ajouter à ma sélection » ticked a spreadsheet (`prepare` now points the Data folder at an empty folder). Agent's expectation wrong: B6, where the widen button cannot repair a carried short question (KBD-19 updated). **Then waived by the owner** (11 October 2026: the rest of her message was truncated and cannot be recovered, and she did not want to re-run a long test): lot 7 is treated as accepted. **Not observed by anyone: B7, C4, C7, D (settings, developer panel, English), E (mode off), F (spreadsheets).** They are covered only by the automated tests and the rendering tests; lot 11's manual protocol should include them. Visible in the application: yes, fully (this lot is the one that makes the base visible). Parts: Preparation, A (`suggest`: nothing narrowed, names asked about, chips from buttons), B (`auto`: the line, the files, the widen, the carried names), C (the `@` list, chips, the outside-selection line), D (settings, developer panel, both languages), E (`off`: the old screen), the only restore, and an optional Part F (two spreadsheets, « Classeur choisi »). Two steps depend on the real embedding model and could not be walked (B7 and F1's wording); they say so. **The agent did not open the application window.**

## Open questions

For the owner:

1. **The `@` list says « 3 fichiers » and the line says « 4 documents sur 30 »** for Jean Dupont: the list counts files that mention him, the line counts files searched, which include `rappel-m-dupont.txt` (it only says « M. Dupont »). Right, or should the list count the same files?
2. **KBD-19.** A short question after a named answer is narrowed to that person in `auto` (the line says so and the button widens). Acceptable for now, with `auto` not the default?
3. **The "which one?" row may offer unconfirmed names** when they fit the words she typed (KBD-11 note). Keep, or label them as unconfirmed?
4. **The `@` list needs a name the base holds as `active`.** A person named only once in untitled text stays a candidate and is not offered. Keep?
5. **The Réglages labels were reworded** (Ciblage intelligent, Désactivé / Suggestions seulement / Automatique). Other words?

Defect rows: **KBD-19** added (deferred); notes on KBD-04, KBD-11, KBD-16, KBD-17, KBD-18. **No row is `blocking`.**

## Next

Lots 8, 9, 10 can start in any order once this lot is merged and tagged `kb-after-lot-07`; **lot 11** (validation: benchmarks, release gate, the fix battery) comes after them. Recommended: **lot 8** (`SESSION-KB-LOT-08-...`, the management dialog, which gives « Voir les connaissances » its destination). What they must know from this lot:

- `knowledge::picker::search_entities` is the only read API of the picker; lot 9's `surface.rs` can sit beside it. The outside-selection group is the only labelled exception to I7: do not add another surface that names a name outside the selection.
- `TargetingReport` candidates carry `documents`/`workbooks`; spans are the user's own words, on screen only.
- `Settings.knowledge_hide_names_in_suggestions` exists and is true by default; lot 9 reads it.
- `ChatEntry` holds `targeting`, `targetingChoice` and `diagnostics`; the history never carries them (`history.test.ts`).
- Source guard reminder: no word of le/la/les/des/une/est/erreur/fichier/dossier/envoi/veuillez/aucune in `src-tauri/src`, comments and test strings included.
- A hook returning a function is run as its cleanup in vitest: write `beforeEach(() => { ... })` with a block when the body calls a mock.

## Git

```text
git status
git add apps/desktop/src apps/desktop/src-tauri/src apps/desktop/src-tauri/tests docs/CLIENT.md docs/DECISIONS.md docs/PRIVACY-AND-SECURITY.md docs/SELECTION-AND-MEMORY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: show targeted searches and let users widen or pick an entity"
git push -u origin feat/kb-targeting-ui
gh pr create --title "feat: show targeted searches and let users widen or pick an entity" --body "<see the final message>" --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-07
git push origin kb-after-lot-07
git switch -c feat/kb-management-dialog
```
