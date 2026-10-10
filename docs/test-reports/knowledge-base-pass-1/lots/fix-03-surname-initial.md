# Fix 3 - The surname-then-initial form (KBD-15)

11 October 2026 · branch `fix/kb-surname-initial` · based on `kb/integration` at `dba57fa` (lot 5 merged, PR #29) · tab "KB fix - Surname and initial"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

A name written surname then initial (`Dupont J`, the form the pilot's own `rdv-mars-2026.xlsx` uses) is now read as such: its surname alone and `J. Dupont` are its aliases, the single letter is no longer a surname, and it is offered as a possible match of `Jean Dupont` (reason `initial`) whichever of the two is stored first, never merged. A question gets the same compatibility as an offer (one full name) or an ambiguity (several), never a pick. A base filed by the previous build is corrected, once, by the next Analyse the user starts, with no document or workbook read again.

## What changed

**Rust, `src-tauri/src/knowledge/`**

- `normalize.rs` - `SurnameInitial` and `surname_then_initial`: **the one place that decides the form** (the last word is one letter, after exactly one word of identity, particles allowed in front; `J. Dupont`, `A B`, `J. P.`, `Jean Dupont J` and one-word names are not it). 3 unit tests.
- `resolve.rs` - `aliases_for` makes, for the form, the initial alias (`J. Dupont`, kind `initial`), the surname alone (and the last word of a particled surname) and the titles seen; no one-letter surname, no `D. J`. `Run::initial_compatibility` (after the exact name and alias, before the fuzzy rule) offers the compatible names in both directions. Two small helpers extracted without a change of behaviour (`split_given_and_surname`, `push_title_forms`); `SCORE_INITIAL` made `pub` for the refresh. 2 unit tests.
- `maintenance.rs` - `ALIAS_GENERATION` (2), `ensure_alias_generation`, `AliasRefresh`: the one-time correction of the people already filed surname then initial. 3 unit tests (the correction, no pair for `Martin A`, a refusal half-way keeps the old state).
- `store.rs` - `people_ending_in_a_letter`, `record_possible_match`. Two small queries.
- `ingest.rs` - `KnowledgeContext::standard` calls `ensure_alias_generation` right after `ensure_phonetic_keys`.

**Tests (`src-tauri/tests/`)**

- `knowledge_resolver.rs` - 7 tests (list below).
- `knowledge_tables.rs` - 3 tests on the pilot's workbooks and on a base filed by the old rules.

**Tooling for the human test (`docs/test-reports/knowledge-base-pass-1/human-tests/`)**

- `show_kb_entities.py` - one new section, « Aliases of people filed as surname + initial ».
- `lot05_helper.py` - one new command, `age-aliases` (puts `Dupont J` back in the state the previous build left, so the correction can be seen on a real index; index only).
- `fix-03-surname-initial.md` - the protocol.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| The initial alias of `Dupont J` is **one** alias, kind `initial`, display `J. Dupont`; the "reordered" `J Dupont` is not made separately | Both normalise to `j dupont` (one row per entity and normalised text). As `reordered` it would be a named variant: `J. Dupont` would resolve to `Dupont J` alone (`decide_exact`) instead of staying a question beside `Jean Dupont`. Test: `an_initial_then_surname_name_is_read_as_before_beside_a_surname_then_initial_name` | `docs/DECISIONS.md`, "Settled by the fix surname then initial" |
| The correction of existing data is `kb_meta.alias_generation`, **not** a raise of `CURRENT_KB_VERSION` | Re-reading a source finds its people already known and does not remake their aliases, and no re-read ever meets `Jean Dupont` as a new name (the moment a possible match is recorded). A version bump would read every document and workbook again and fix nothing. The generation check is next to the phonetic-key check, in the context an Analyse builds, in one transaction | same |
| A query for the exact full name `Jean Dupont` still resolves to that entity alone | An exact name wins (master section 7). Whether a reduction should widen to the files of the entity's possible matches is lot 6's decision; the pair is recorded for it | same, and "Next" below |
| The two-full-names case answers `Ambiguous` in a question and a `PossibleMatch` listing both at ingestion | Same convention as `decide_initial` for `J. Dupont`; both are "a question, never a pick" | `a_surname_then_initial_never_picks_between_two_full_names` |
| Only a person is read this way | An organisation named `Dupont J` is a name; type checked in the resolver, the alias maker is already inside the person branch | `only_a_person_is_read_as_a_surname_and_an_initial` |

## Deviations

- The lot file asks for "the reordered form is `J Dupont`". It is the same normalised text as the initial form (see the first decision), so one alias carries both; the display is `J. Dupont`.
- The lot file says "raise `CURRENT_KB_VERSION` (or the alias generation)". The alias generation was chosen, for the reason above.
- The master section 7 and the lot file say the pair is recorded when a name is **new**. For data that exists already no name is new, so the correction records the pairs itself (`ensure_alias_generation`); it uses the same compatibility and the same reason and score.

## Invariants

| Invariant | Proof |
| --- | --- |
| An initial and a surname never links without support; fuzzy or compatible never merges (master section 7) | `a_surname_then_initial_is_a_possible_match_of_the_full_name_in_both_orders` (resolver: even with the source vouching, still a possible match), same name in the Data pass: two entities, two sources |
| A person stays a person; a module changes no type | Only the person type is read this way (`only_a_person_is_read_as_a_surname_and_an_initial`); `the_numbers_the_human_test_quotes_hold` and the module tests unchanged |
| No costly silent background work | `ensure_alias_generation` runs only in the context of an Analyse (`KnowledgeContext::standard`), reads no document, and a test proves no workbook is read again (`a_base_read_before_this_fix_is_corrected_by_the_next_data_analyse_and_only_then`) |
| Do not break lot 4 | `knowledge_resolver`, `knowledge_documents`, `knowledge_tables`, `knowledge_names` unchanged and green; `the_initial_surname_form_keeps_the_aliases_it_has_always_had` guards `J. Dupont`, `Jean Dupont`, `A B` |
| I4/I7 (inside the selection first, a failed lookup falls back) | The new step uses `keep` (selection applied first) and the existing failed-lookup path; `an_entity_outside_the_selection...` family unchanged |

## Verification

| Command | Result | Before the fix |
| --- | --- | --- |
| `cargo test --lib` | **697 passed**, 0 failed, 2 ignored | 689 |
| `cargo test --test knowledge_resolver` | **53 passed** | 46 |
| `cargo test --test knowledge_tables` | **50 passed**, 3 ignored | 47 |
| `cargo test --test knowledge_documents` / `knowledge_names` / `knowledge_packs` / `knowledge_store` | 45 / 26 / 21 / 19 passed, unchanged | same |
| `cargo test` (every target) | **1060 passed**, 0 failed, 13 ignored | 1042 passed (lot 5 report) |
| `pnpm run test` | 362 passed (28 files), the source guard included | 362 |
| `rustfmt --edition 2021 --check` on the files touched | clean (never plain `cargo fmt`) | |
| `cargo clippy --all-targets` | lib: **21 warnings = the baseline**, none in a file this fix touched (the other targets only repeat existing ones) | 21 |
| `pnpm run build` | OK (`tsc --noEmit` and `vite build`; the chunk-size notice is old) | OK |

Not run: `pytest` and `ruff` (the server is untouched), macOS (no machine; nothing platform-specific was added, the new code is pure string logic and SQL).

## Measured

Nothing heavy was added. Per new full name at ingestion, one extra indexed lookup of an alias (`by_alias`); the in-memory lookup used inside a pass is a hash lookup. `extraction_cost_on_fifty_thousand_rows_of_eight_columns` and `a_hundred_thousand_rows_are_read_in_a_bounded_time` still pass. The correction of an old base costs one query on the people whose name ends in a letter plus a few rows per person; run on the pilot's fictional index (9 such people) it is instantaneous.

## Not done

- Widening a question's scope to the files of an entity's possible matches (lot 6).
- A "did you mean" in the interface (lot 7), and the management of possible matches (lot 8).
- A surname-then-initial name in a **document**: the text extractor produces `Dupont J` only when a source writes it so; its behaviour is unchanged, the rule applies to whatever name reaches the resolver.
- A given name followed by one letter (`Jean M`) cannot be told from a surname and an initial; it is read as the latter and only ever offered as a question.

## Documentation

`docs/DECISIONS.md` ("Settled by the fix surname then initial"), `defects-register.md` (KBD-15 `fixed`), this report, the human test, `show_kb_entities.py`, `lot05_helper.py`. Not touched: `docs/ARCHITECTURE.md` (no new piece), `docs/PRIVACY-AND-SECURITY.md` (no new data), `docs/LANGUAGE-AND-LOCALE.md`.

## Human test

`human-tests/fix-03-surname-initial.md` - **run by the owner on 11 October 2026, every step matched, no finding** (pull request #30). Partly visible: the script shows the possible match and the aliases; the interface shows nothing new (no "did you mean" before lot 7). The agent did not open the application window.

## Open questions

None blocking. Rows changed in the register: **KBD-15** (`fixed`). For the owner, later: should lot 6 widen a reduction to the files of an entity's possible matches (so a question about `Jean Dupont` also keeps `rdv-mars-2026.xlsx`)? It is lot 6's decision; this fix only records the pair.

## Next

Lot 6, `docs/SESSION-KB-LOT-06-Targeted-scope.md`, branch `feat/kb-targeted-scope`, after this fix is merged and tagged `kb-after-fix-03`. What it must know:

- `kb_possible_matches` rows with reason `initial` link a `Surname Initial` person to the full names that fit it. An exact full-name question still resolves to the full name's entity alone; the zero-candidate fallback does not widen it. If `auto` must keep `rdv-mars-2026.xlsx` for a question on `Jean Dupont`, lot 6 reads these pairs (`open` ones) when it builds a scope.
- A surname alone (`Dupont`) is now `Ambiguous` when both forms are in the selection (chips, no reduction); inside a selection holding only one of them it resolves as before.
- `knowledge::maintenance::ALIAS_GENERATION` is the lever for any future change of `aliases_for` that must reach existing entities: raise it and add the correction to `ensure_alias_generation`.
- `normalize::surname_then_initial` is the only place that reads the form; lot 6's name detection must call it rather than test "last word is one letter" again.

## Git

See the final message to the owner (the block is given in chat).
