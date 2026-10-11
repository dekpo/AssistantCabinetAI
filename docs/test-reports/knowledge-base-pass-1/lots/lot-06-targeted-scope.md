# Lot 6 - Targeted scope

11 October 2026 · branch `feat/kb-targeted-scope` · based on `kb/integration` at `83b6dfa` (tag `kb-after-fix-03`, lots 4 and 5 and the fix "surname then initial" merged) · tab "KB 6 - Targeted scope"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

A question is now read against the knowledge base, inside the files the user selected, in about 2 to 15 milliseconds and with no request to the AI: the names it holds are found (a full name, a surname, a title and a surname, an identifier, a chip, the previous answer's entities for a short follow-up), a name that fits several people is asked about and never picked, and the files of the selection that mention them are worked out. In `suggest` (the default) the search is exactly the one that runs with the base off, and the base only records what it *would* have searched; in `auto`, or when the user picked an entity chip, the search covers only those files (plus the files whose text holds the name in another form, the files the base has never read, and the files of an open possible match), is repeated once on the whole selection if it came back empty or far from the question in meaning, and the engine of the spreadsheets is given a shorter list of workbooks built from the user's own selection. Nothing the question says changes the base, nothing the base knows reaches the model or a log, and the interface is not changed beyond the type definitions and one false label.

## What changed

**Rust, new (`src-tauri/src/knowledge/`)**

- `query.rs` - `QueryEntityDetector`, `QueryAnalysis`, `QueryEntity`, `SelectedFile`: chips, runs of one to five words looked up with one query then resolved in query mode inside the selection, the fuzzy fallback (possible matches only), identifiers by pack scheme, carry-over, the files and share of the selection per entity, the surface forms for the lexical net. 3 unit tests.
- `reduce.rs` - `reduce`, `ReductionPlan`, `SideReduction`, `SkipReason`, `TargetingPolicy`, `compare_with_cited` (the shadow comparison and the miss classification). 4 unit tests.
- `targeting.rs` - the Tauri-free orchestration: `plan`, `search_documents` (reduced search, one widening), `narrowed_data_selection`, `decorate_which_workbook`, `note_documents`/`note_workbooks`, `TargetingReport`, `TargetingRequest`, `QueryVocabulary`. 3 unit tests. **Not in the lot file's list**; the orchestration had to live where a test can reach it.

**Rust, existing files (additive unless said)**

- `knowledge/store.rs` - `sources_in_scope`, `known_name_forms`, `open_possible_partners`, `sheets_for_entities`; `ensure_schema` adds seven columns to `kb_diagnostics`; **`source_ids_for_scope` (lot 3) is rewritten to read the pairs once** (see Decisions).
- `knowledge/diagnostics.rs` - `RetrievalDiagnostics.targeting` (`TargetingDiagnostics`, `QueryEntityDiagnostic`, `ShadowDiagnostics`), `record_question` (the row and the ring of 2 000), `aggregate`/`KnowledgeAggregates`.
- `knowledge/resolve.rs` - `initial_surname_key_of_full_name` is `pub(crate)`. `knowledge/mod.rs` - three `pub mod` lines.
- `retrieval.rs` - `search_scoped_counted_with_terms`, `search_per_document_counted_with_terms` (extra surface forms for the lexical part only), `SearchOutcome.best_vector_score`. The existing functions delegate with no extra term.
- `index_store.rs` - `search_lexical_terms_in`, `files_matching_terms_in`, `entity_terms_expression`.
- `settings.rs` - five settings (`knowledge_min_scope_files`, `_max_selectivity`, `_min_reduction`, `_max_candidates`, `_widen_below_score`), `Settings::targeting_policy`. `Settings` loses `Eq`.
- `tabular_answer.rs` - `WorkbookHint`, `WhichWorkbook.hints`, `selected_workbook_files`, `names_a_workbook`. One unit test literal gained `hints: Vec::new()` (no assertion moved).
- `conversation.rs` - `FOLLOW_UP_MAX_WORDS` is `pub`.
- `knowledge/secret.rs` - `IdentifierKey::load_if_present` (a question reads the key and never makes one). 1 unit test.
- `commands.rs` - `ask_with_sources(.., targeting)`, `AskAnswer.targeting`, `ChatStreamEvent::Targeting`, the knowledge step in `sourced_answer`, `tabular_tier`, `mixed_data_only_tier` and `mixed_tier`, `TargetingSetup`, a vocabulary kept in `AppState`, `knowledge_diagnostics` (registered in `lib.rs`).

**Interface (`apps/desktop/src`)**

- `lib/ipc.ts` - types only: `TargetingRequest`, `TargetingReport`, `TargetingDiagnostics`, `KnowledgeAggregates`, `WorkbookHint`, the `targeting` stream event, `askWithSources(.., targeting?)`, `knowledgeDiagnostics()`. The stream handler ignores events it does not know.
- `locales/{fr-FR,en-US}.json` - **one label changed in each**: « Automatique (identique à « Activée » pour l'instant) » was false from this lot on; it now reads « Automatique (recherche seulement dans les fichiers cités par la question) » / « Automatic (searches only the files the question names) ».

**Tests and tools**

- `tests/knowledge_targeting.rs` (56 tests + 1 ignored printer), `tests/knowledge_isolation.rs` (8), `tests/knowledge_targeting_bench.rs` (2 ignored), `tests/common/corpus.rs` (the invented clinic), `tests/common/knowledge.rs` (lab helpers).
- `lots/lot-06-widen-calibration.py` (the measurement behind 0.65), `human-tests/lot06_helper.py`, `human-tests/show_kb_diagnostics.py`.

## Decisions taken

All recorded in `docs/DECISIONS.md`, "Settled by KB lot 6". The ones to know:

| Decision | Reason |
| --- | --- |
| **Any ambiguity inside the selection stops the reduction** (unless the user picked a chip), not only one "on the main span" | A question has no main span a rule can find, and a name left out of the reduction is a file lost |
| **A file the base has not read is always kept** (no source row under its current content, or read by no extractor), beyond the cap | What the base does not know cannot be a reason to leave a file out; it also covers a document changed since its last Analyse |
| **A reduction keeps the files of the entity's open possible matches** | The decision KBD-15 left to this lot: `Dupont J` and `Jean Dupont` are the same person until the user says otherwise |
| **A surname alone, or a title and a surname, is searched by the lexical net only when no other entity carries it _inside the selection_** | Otherwise `martin` pulls in every other Martin and undoes the reduction (`the_forms_of_a_name_shared_with_other_people_do_not_widen_the_net`). "Inside the selection" was added after the owner's first run: a patient of a workbook she had not ticked made `dupont` look shared and `rappel-m-dupont.txt` was missed (`a_surname_shared_only_with_a_workbook_outside_the_selection_is_still_searched`) |
| **Widening reads the closest passage by meaning, without the lexical bonus** (`best_vector_score`), and an "each document" question widens only when empty | A reduced set always carries the bonus for the name it holds; a coverage list has low scores by nature |
| **`knowledge_widen_below_score` = 0.65**, measured | Unrelated questions reach 0.555 to 0.645 against the passages, questions whose passage is there 0.698 to 0.843 (`nomic-embed-text`). The scale belongs to the embedding model |
| **A question writes nothing, not even the identifier key** (`load_if_present`, not `load_or_create`) | With no key there is no hashed identifier to recognise; creating a file at question time would be a write the user did not start |
| Two skip reasons added: `no_entity`, `user_widened` | The question names nothing the base knows is not the same as "nothing sure enough"; the widen action is the old path |
| Chips apply at any size and whatever the entity's share of the selection | Explicit user intent |
| In `suggest` the buttons of "which workbook?" are ordered and annotated by the base | The lot file asks for it; the choice stays the user's |
| The auto-mode label in « Réglages » was changed | It said "identical to Activée for now", which this lot makes false |

## Deviations

- **From the lot file:** (1) `targeting.rs` and `tests/common/corpus.rs` are new files. (2) "Calibrate in lot 11" in the lot file, "calibrate in lot 6" in the launcher and the master's table: it was measured here (0.65) and lot 11 checks it on real documents. (3) The widening score is the vector score without the bonus, not "the best score". (4) The ambiguity gate is stricter, as above. (5) The `RetrievalDiagnostics` fields the lot file lists are nested under `targeting` instead of being flat. (6) `knowledge_hide_names_in_suggestions` is not added (lots 7 and 9). (7) The detector looks the runs of words up with one query for the **names and aliases**; identifiers are matched by scanning the question with the packs' schemes, then resolved. (8) The shadow comparison of the mixed tier covers the documents only; the answering workbook is not known there.
- **From the master:** none found. "What the code looks like today" matches the code for the symbols this lot touches (`sourced_answer`, the tiers, `retrieval.rs`, `WhichWorkbook`, `source_ids_for_scope`).
- **Existing code changed beyond the lot's list:** `source_ids_for_scope` (a quadratic join, see Measured), `Settings` (no `Eq`), one `tabular_answer` test literal, the two locale labels.

## Invariants

| Invariant | Proof |
| --- | --- |
| I1 - the candidates are a subset of the selection | `reduced_scope_is_always_a_subset_of_the_selection` (120 random selections, searched too), `the_reducer_drops_a_path_outside_the_selection_even_when_an_entity_lists_it` (200 random hand-made analyses with foreign paths), `a_selection_of_three_files_never_shows_a_fourth_that_names_the_same_person`, `a_file_that_left_the_folder_is_never_used_even_if_the_index_still_knows_it`, `narrowing_the_workbooks_builds_a_subset_of_the_entries_the_user_made` |
| I2 - a named file is never reduced | `a_named_file_is_never_reduced` (also with a chip, also the search of one file), `a_named_workbook_is_never_reduced` |
| I3 - nothing a question says is learned | `asking_a_question_does_not_change_the_knowledge_base` (five kinds of request), `a_question_with_a_never_seen_full_name_creates_no_entity`, `a_model_answer_does_not_change_the_knowledge_base`, `the_history_of_a_conversation_does_not_change_the_knowledge_base` (a snapshot of every `kb_` table but `kb_diagnostics`, `tests/knowledge_isolation.rs`) |
| I4 - every failure falls back | `zero_candidates_fall_back_to_the_full_selection`, `a_knowledge_lookup_failure_falls_back` (a broken view), `a_writer_holding_the_database_does_not_stop_a_question`, `the_widen_action_gives_the_evidence_the_old_path_gives` |
| I5 - the engine is not altered; no sheet restriction written | `the_knowledge_layer_never_writes_sheet_restrictions` (lot 5, it reads the new files too), `the_engine_answers_the_same_from_the_narrowed_selection_as_from_the_user_naming_the_workbook`, `a_user_named_sheet_the_base_did_not_hint_still_answers` |
| I6 - the same path in both folders | `the_same_relative_path_in_both_folders_stays_two_sources_in_a_reduction`, `documents_and_workbooks_are_decided_independently`, lot 5's store test |
| I7 - nothing outside the selection is revealed | `ambiguity_chips_only_offer_entities_present_in_the_selection` |
| I8 - nothing the base holds reaches a model or a log | `no_knowledge_text_reaches_a_model_facing_string`, `the_prompt_is_built_from_the_same_pieces_whatever_the_knowledge_mode`, `diagnostics_rows_and_the_timing_log_contain_only_numbers_and_codes`, `no_name_the_base_holds_is_in_a_row_the_questions_write` |
| I9 - model-facing strings stay neutral | No model-facing string added; the existing guards pass unchanged |
| I10 - machine codes, no French in `src-tauri` | `pnpm run test` (the source guard reads the three new modules and every file touched; catalogue parity for the changed label) |
| I11, I14 | Not touched: this lot writes no file and does not change ingestion |
| I12 - the webview never calls the server | Every operation is a Tauri command (`ask_with_sources`, `knowledge_diagnostics`) |
| I13 - no `cfg`, no Windows path literal | No `cfg`; the tests use temporary folders. **Not run on macOS** |

## Verification

Windows 11, debug profile unless said. The baseline is the figure of the fix 3 report (`kb/integration` at `83b6dfa`); a run of my own before the first edit passed every target but was overlapped by my first edits at its doc-test stage.

| Command | Baseline | After |
| --- | --- | --- |
| `cargo test` (all targets, from `apps\desktop\src-tauri`) | 1060 passed, 0 failed (697 lib) | **1138 passed, 0 failed, 16 ignored** (+78: 14 lib, 56 in `knowledge_targeting.rs`, 8 in `knowledge_isolation.rs`) |
| of which `--lib` (what CI runs) | 697 | **711** |
| `cargo clippy --lib` | 21 warnings | **21** (one new was fixed: the size of `ChatStreamEvent::Targeting`; two `too_many_arguments` are allowed with a comment, as the other builders are) |
| `rustfmt --edition 2021 --check` | - | clean on `knowledge/mod.rs` (which formats every module under it), `retrieval.rs`, `settings.rs`, the three new test files and `tests/common/*`; `commands.rs` (8 hunks, 9 on `HEAD`), `index_store.rs` (4, same), `tabular_answer.rs` (43, same) and `conversation.rs` (1, same) have only hunks that were already there; no plain `cargo fmt` was run |
| `pnpm run test` (from `apps\desktop`) | 362 passed (28 files) | **365 passed** (28 files; the source guard has three more files to read) |
| `pnpm run build` | OK | OK |
| `apps/server` `pytest` | not run | not run: nothing there changed |

Not run: the application window, a gateway turn of `ask_with_sources` (the wiring in `commands.rs` was compiled and read, and every function it calls is tested; the glue itself is not), macOS, a release `tauri build`.

## Measured

Windows 11, the owner's development PC. Release profile, `cargo test --release --test knowledge_targeting_bench -- --ignored --nocapture`, fake vectors, 15 rounds, medians. **The generation time, which dominates a real answer, is not measured and not changed.**

| Files in the selection | Reading the question and deciding | Search, whole selection | Search, reduced | Chunks scored, whole / reduced |
| --- | --- | --- | --- | --- |
| 10 | 1.70 ms | 0.57 ms | 0.40 ms | 10 / 1 |
| 20 | 1.78 ms | 0.96 ms | 0.43 ms | 20 / 1 |
| 30 | 2.24 ms | 1.58 ms | 0.79 ms | 30 / 2 |
| 50 | 2.70 ms | 3.11 ms | 0.86 ms | 50 / 3 |
| 100 | 2.05 ms | 4.72 ms | 0.72 ms | 100 / 5 |
| 500 | 4.23 ms | 46 ms | 2.8 ms | 500 / 25 |
| 1 000 | 14.5 ms | 4 530 ms | 259 ms | 1 000 / 50 |

- **The lookup against a base of 10 000 more entities: 3.65 ms** (budget 20 ms).
- **A defect found by the benchmark in this lot's own first version, fixed:** the lookup took 542 ms at 1 000 files because `source_ids_for_scope` joined the sources onto `json_each` and the planner parsed the whole array once per source. The pairs are now read once; 14.5 ms.
- **KBD-16, recorded, not fixed:** the whole-selection search at 1 000 files costs 4.5 s because `chunks_for_document` scans the table once per file (no index on `chunks.relative_path`).
- **The widening threshold**, `python docs\test-reports\knowledge-base-pass-1\lots\lot-06-widen-calibration.py` (9 invented passages, 12 questions plus 6 unrelated ones, `nomic-embed-text` through the gateway): unrelated 0.555 to 0.645; the reduced set holds the answer 0.698 to 0.843 (median 0.773); the reduced set names the person but the answer is elsewhere 0.673 to 0.764. **0.65 is the threshold that separates the first two; nothing separates the third from the second (KBD-17).**
- **Real documents:** the thirty invented documents of the human test: the first question (a doctor) keeps 4 of 30 files (86 % left out), the second (a Martin) is refused as ambiguous, the third (a full name) keeps 1 of 30 (96 %).

## Not done

- The targeting line, the widen button, the chips, the picker, the settings group (lot 7). The suggestions and `knowledge_hide_names_in_suggestions` (lots 7 and 9).
- A relative widening rule (KBD-17), the post-KB fix of KBD-16.
- A shadow comparison of the answering workbook in the mixed tier.
- Running the application window; macOS.
- The `carryOver` entities are never sent by the interface yet (lot 7); the Rust side is tested.

## Documentation

`docs/DECISIONS.md` ("Settled by KB lot 6"), `docs/RETRIEVAL.md` ("Targeting inside the selection"), `docs/SELECTION-AND-MEMORY.md` ("Targeting inside the selection": the subset rule, the modes, memory isolation), `docs/PRIVACY-AND-SECURITY.md` (the row "A question is asked"), `docs/ARCHITECTURE.md` (the question-time paragraph), `defects-register.md` (KBD-16, KBD-17 and the notes), this report, the human test and its two scripts, the calibration script. Not touched: `docs/CLIENT.md` (no interface yet), `docs/LANGUAGE-AND-LOCALE.md`.

## Human test

`docs/test-reports/knowledge-base-pass-1/human-tests/lot-06-targeted-scope.md` - **run once by the owner on 11 October 2026 (first version); a re-run of B1, C2, C3 and Part E is proposed.** What matched: B2 to B5, C1, C4 to C7, D2 to D4. What did not, and which side was wrong: four expectations of the protocol (an index that already knew `Jean Dupont`; `Pierre Martin` is read by the router as a file name, KBD-18; the timing log writes a line in `off` mode; and the **order of the parts**: a `restore` at the end of Part D put her own settings back, so Part E ran on her own folder), and **one product defect** (3 files kept instead of 4, fixed in this branch, see Decisions). C6 showed the known limit on the real application: `auto` answered that Dr Dupont does not mention the photocopier although `note-photocopieuse.txt` does (KBD-17). The protocol was rewritten (parts reordered, each with "You start with" and "You leave with", one final restore), and the rule that caused the error is now global (`AGENTS.md`, "Human tests"; `kb-programme.mdc` item 9; workflow section 5; the human-tests README). **The agent did not open the application window.**

## Open questions

For the owner:

1. **Is 0.65 enough, given KBD-17?** It catches a reduced set unrelated to the question and cannot catch one that names the right person but lacks the answer. `auto` is not the default, the lexical net and the unread files soften it, and lot 7's button is the remedy. Step C6 of the human test shows the limit on your own eyes. A relative rule is lot 11's decision, with the recall measured on your documents.
2. **Any ambiguity stops the reduction** (stricter than the lot file). Keep?
3. **Files the base has never read are always kept**, beyond the cap of 15. Keep?
4. **The Réglages label of `auto` was changed** because the old one was false. Say if you want other words.
5. ~~After step C6~~ Answered by the run: `auto` did not find it (`widened False`). Does that change your view on question 1?

Defect rows: **KBD-16** and **KBD-17** added (both `deferred`); notes on KBD-01, KBD-04, KBD-11, KBD-12, KBD-15. **No row is `blocking`.**

## Next

**Lot 7 - `docs/SESSION-KB-LOT-07-Targeting-in-the-interface.md`**, tab "KB 7 - Targeting in the interface", branch `feat/kb-targeting-ui`, from `kb/integration` after this lot is merged and tagged `kb-after-lot-06`. Lots 8 (needs 1 to 4) and 9 and 10 can also start; **recommended: 7**, which makes this lot visible. What it must know from this lot:

- The wire: `ChatStreamEvent::Targeting { report }` arrives just before `Sources` (only when there is something to tell); `AskAnswer.targeting`; the request is `askWithSources(.., targeting?)` with `mode: "off"` for the widen button, `entityIds` for chips, `carryOver` for the previous answer's entity ids (held by the client, honoured only for a question of eight words or fewer that names nobody). Types are in `lib/ipc.ts`.
- `TargetingReport.entities` lists resolved entities, **candidates included** (confidence at most 0.40): hide them from chips and the picker (KBD-11). `ambiguities[]` and `possibles[]` hold entities of the selection only.
- `reason` is a machine code (`ambiguous`, `no_entity`, ...); `requestedBy` is `auto` or `chip`; `widened` is `none` or `auto`; `chosenWorkbook` is set when one workbook was left.
- In `suggest`, `report` is `None` unless names need a question; an unapplied reduction is invisible. `TabularAnswer::WhichWorkbook.hints` is machine data to annotate the buttons.
- A chip is an entity id, never a sentence appended to the question (KBD-04).
- `knowledge_diagnostics(limit)` is ready for the developer panel (`show_diagnostics`).
- The `auto` label in « Réglages » is already corrected; the `Avancé` group should expose the five new settings.
- KBD-17: the widen button matters more than the automatic widening.

## Git

```text
git status
git add apps/desktop/src-tauri/src apps/desktop/src-tauri/tests apps/desktop/src/lib/ipc.ts apps/desktop/src/locales docs/ARCHITECTURE.md docs/DECISIONS.md docs/PRIVACY-AND-SECURITY.md docs/RETRIEVAL.md docs/SELECTION-AND-MEMORY.md docs/test-reports/knowledge-base-pass-1
git commit -m "feat: reduce the search to files that match the question's entities"
git push -u origin feat/kb-targeted-scope
gh pr create --title "feat: reduce the search to files that match the question's entities" --body "<see the final message>" --base kb/integration
```

After the owner has accepted the human test and CI is green:

```text
gh pr merge <number> --merge
git switch kb/integration
git pull
git tag kb-after-lot-06
git push origin kb-after-lot-06
git switch -c feat/kb-targeting-ui
```
