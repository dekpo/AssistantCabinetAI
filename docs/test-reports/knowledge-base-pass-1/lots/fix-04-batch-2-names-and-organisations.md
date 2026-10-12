# Fix 4, batch 2 - The extractor reads what a real library writes (F-06, F-07, F-08, F-09, F-10, F-04)

12 October 2026 · branch `fix/kb-names-and-organisations` · based on `kb-after-fix-04-batch-1` (`fc1a425`, PR #37) · tab "KB fix 4 - Batch 2 (names and organisations)"

Written for the developer or agent of the next batches, and for the owner. English; French sentences are quoted test data.

## Summary

Six fixes of the battery, each proved by a test written first and seen failing (`tests/knowledge_fix_names.rs`, 27 tests, 3 of them ignored prints). The hospitals and insurers a Swiss or French practice writes without a marker word (`CHUV`, `HUG`, `SUVA`, `URSSAF`, `Hospices Civils de Lyon`) are organisations, with the acronym and the full name as one entity; the recipient of a letter alone on a line is found; a surname with an apostrophe (`D'Amico`) is one person; `Roy Lucas` and `Lucas Roy` are no longer one entity; a street or an address fragment is no longer a person. On the sandbox at 200 documents the organisations found in the documents go from **66.0 % to 98.0 %**, the persons from **92.3 % to 99.0 %**, the names never found from **25 to 2**, the false merges from **2 to 0** (tier 100 and 200; **1** at tier 30, see "Not done"), and the recall of the files that hold the answer on the real stack from **90.4 / 95.6 / 95.3 %** to **93.3 / 97.0 / 96.5 %** at 30 / 100 / 200 documents (gate 98 %: still not reached).

## What changed

**Rust, `apps/desktop/src-tauri/src/`**

- `knowledge/mod.rs` - `CURRENT_KB_VERSION` 2 to 3; methods `organization_known` (0.80, trusted at once) and `block_name` (0.60, trusted at once).
- `knowledge/extract/text.rs` (extractor version 2) - known organisations from the packs (a full name, or a short form written exactly as the pack writes it; the entity carries the full name); lower case words after an organisation's opening word (`Hopitaux universitaires de Zurich`); an ampersand joins two halves of a name; `l'Etude` is read without its article; sentence-initial prepositions, articles and particles open no name; up to two particles in a row (`de la`, `van den`); a name alone on a line (a salutation above, or an address below); a break inside a wrapped paragraph is a space; a street or an address fragment is no person; `settle` credits the pattern that found a name the base already knew.
- `knowledge/normalize.rs` - `apostrophe_joins_a_name` (one function for the normaliser, the extractor and the question reader: `D'`, `N'`, `O'` before a capital belong to the surname), `writes_surname_first`, `given_name_first`.
- `knowledge/resolve.rs` - a surname in capitals first is read given name first; a plain-case reordered alias inside one folder is a `PossibleMatch` of reason `swapped_order`; `EntityLookup::names_in_the_same_folder`.
- `knowledge/ingest.rs` - canonical names and groups use the given-name-first reading; the name of an organisation known by its legal form is also an alias without it; the titles seen with a known person are added to her names (`Dr Dupont`); the acronym of a listed organisation is its alias.
- `knowledge/store.rs` - `kb_entities.promoted_from_candidate`, `promote_candidates` marks, `demote_candidates` runs with every collection of orphans; `delete_entity` records the workbooks that named the entity and `restore_entity` makes them due; `open_possible_partners` ignores `swapped_order` pairs for documents (tables keep them); `entity_has_mention_in_domain`.
- `knowledge/reduce.rs`, `knowledge/query.rs` - the reduction asks for the partners by side; the lexical net does not search a reordered form that is another person's name, nor a surname (alone or after a title) that is another person's given name.
- `knowledge/packs.rs` + `resources/knowledge/` - new fields `org_names` (a full name and its short forms) and `address_markers`, validated and read as **type vocabulary** (every shipped pack's, whatever modules are active); lists in `health-ch` (27 names), `health-fr` (12), `accounting-fr` (2); address markers and function words in the base packs (French and English); `chu`, `chru`, `ordre des médecins` as opening words in `health-fr`.

**Client** - `apps/desktop/src/locales/{fr-FR,en-US}.json`: the method labels `organization_known`, `block_name` and the reason `swapped_order`.

**Tests** - `tests/knowledge_fix_names.rs` (new); `tests/knowledge_packs.rs` (the new fields, a test for `org_names`); `tests/knowledge_validation.rs` (the measure matches a swapped pair by its ordered name); three expectations changed on purpose, see "Deviations".

**Documentation** - `docs/DECISIONS.md` ("Settled by KB fix 4, batch 2"), `docs/KNOWLEDGE-BASE.md`, `defects-register.md` (11 rows fixed, KBD-44 to KBD-46 added), `05-branches-and-stages.md`.

**Human test** - `human-tests/fix-04-batch-2-names-and-organisations.md`.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| Unlisted acronyms are **not** read as organisations (the battery proposed "3 to 6 capitals is a candidate") | Two files that agree promote a candidate to active: `IRM`, `TSH`, `PDF`, `TVA` would become active organisations | DECISIONS, KBD-46 |
| The battery's "type an untitled run as `term`" is **not** done | A person met in a sentence is a person candidate; the streets and address fragments are filtered by their markers instead | DECISIONS, KBD-11 |
| Plain-case swapped order is two people **inside one folder**, one person across folders; a pair is never kept in a document reduction, but is kept for the tables | A table that writes `Dupont Jean` and a letter that writes `Jean Dupont` are the usual way one person is written twice (the first version, without the folder rule, made 16 duplicates in 170 persons); the engine filters rows by the name, so keeping the other workbook is harmless | DECISIONS, KBD-25 |
| A wrapped name is joined only in a wrapped paragraph (three lines at 85 % of the longest, 60 characters at least) | `chunking::chunk` removes blank lines, so a letterhead or an invoice looks like a wrapped line; a looser rule made `Annemasse Solution`, `Annemasse Tel` (31 junk entities at 200 documents) | DECISIONS, KBD-44 |
| The titles seen with a known person are added to her names | A recipient alone on a line became an entity before the letters that called her `Dr`, and the net lost `Dr Dupont` (5 relevant files lost at tier 200, then back) | DECISIONS |
| Lists of organisations are pack data and type vocabulary | A module never changes a type; they are data per country | DECISIONS, `resources/README` unchanged |

## Deviations

- **Three expectations of earlier tests changed on purpose** (the tests encoded what this batch changes): `extract::text::tests::a_title_before_a_known_name_is_kept_as_a_title_seen` (the method is now the title's, not the list's: F-10); `tests/knowledge_fix_swiss_first.rs::a_commune_of_a_table_never_narrows_the_documents` (the hospital is now a known organisation, so its question is narrowed to its own two files; the commune still adds nothing); `tests/knowledge_targeting.rs::people_who_share_a_surname_are_never_taken_for_one_person` (`Dr Dupont` is the only Dupont ever written with `Dr`, so it now resolves to him; `Dupont` and `J. Dupont` stay questions).
- The battery's F-07 text proposes "type an untitled run as `term`" and F-06 "an acronym of 3 to 6 capitals": neither done (see Decisions).
- "Seen failing first": every proving test of `knowledge_fix_names.rs` failed on the code of the previous batch, except four that guard against the first versions of my own rules, each seen failing with the rule switched back (`a_letterhead_or_an_invoice_line_is_not_joined_to_the_next_one`, `a_question_for_one_of_two_swapped_names_searches_only_that_person`, `a_question_for_a_name_keeps_the_workbook_that_writes_the_other_order`, `a_title_seen_with_a_known_person_teaches_the_base_her_title_form`). The two apostrophe tests pass on the previous code in their small world; the sandbox's Q054 and Q081 are the failing proof (`explain_one_question`: the entity of `Helene D'Amico` answered both).

## Invariants

I1 to I14 unchanged: `knowledge_isolation`, `knowledge_targeting`, `knowledge_validation` (0 file outside the selection, 0 leak at all three tiers); `reduced_scope_is_always_a_subset_of_the_selection` unchanged. I3: nothing a question says is learned (the new reads are at Analyse only). Model-facing strings: none added (guard tests untouched). Rust holds no French literal (`pnpm run test` language guard green).

## Verification (Windows, this PC)

| Command | Result | Before the batch |
| --- | --- | --- |
| `cargo test --lib --tests --no-fail-fast` | **1 294 passed**, 0 failed, 41 ignored | 1 269 |
| `pnpm run test` | **627 passed** | 627 |
| `cargo clippy --lib` | 21 findings | 21 |
| `rustfmt --edition 2021 --check` on the touched files | clean (`resolve.rs` keeps its one finding of `HEAD`) | |
| `pnpm run build` | OK | OK |
| macOS | **not run** (Mac mini not delivered); no `cfg`, no platform path, no new crate | |

## Measured - before / after

Machine: this PC (Windows), release build, `fixtures/knowledge-sandbox`, mode `Auto`, fake vectors and a perfect text recognition for the structural replay. Before: `measurements/fix-04-batch-1/`. After: `measurements/fix-04-batch-2/`.

| Figure | Tier 30 before | after | Tier 100 before | after | Tier 200 before | after |
| --- | --- | --- | --- | --- | --- | --- |
| Persons of the documents found | 26 / 26 | 26 / 26 | 76 / 78 | **78 / 78** | 96 / 104 | **103 / 104** |
| Organisations of the documents found | 12 / 19 | **18 / 19** | 22 / 34 | **33 / 34** | 33 / 50 | **49 / 50** |
| Link recall, organisations (documents) | 69.0 % | 96.6 % | 63.3 % | 95.6 % | 71.5 % | **95.3 %** |
| Link precision, persons (documents) | 100.0 % | 100.0 % | 97.3 % | 100.0 % | 98.1 % | **100.0 %** |
| Names of the documents never found | 7 | 1 | 14 | 1 | 25 | **2** |
| False merges | 2 | **1** | 2 | **0** | 2 | **0** |
| Entities matching no invented name (active / candidate) | 2 / 2 | 1 / 0 | 4 / 7 | 3 / 3 | 5 / 5 | 2 / 3 |
| Questions whose documents were reduced | 61 | 67 | 108 | 117 | 112 | 124 |
| **Recall of the reduced set** (files that hold the answer) | 88.5 % (77/87) | **91.6 % (87/95)** | 82.5 % (227/275) | 84.5 % (251/297) | 96.3 % (312/324) | **96.7 % (348/360)** |
| Wrong entity picked | 0 | 0 | 2 | **0** | 2 | **0** |
| Leaks | 0 | 0 | 0 | 0 | 0 | 0 |
| Relevant files lost, `capped` | 0 | 0 | 38 | 38 | 0 | 0 |
| Relevant files lost, `weak_form_only` / `carry_over` | 8 / 2 | 6 / 2 | 7 / 3 | 5 / 3 | 5 / 7 | 5 / 7 |

Knowledge lookup per question, median / p95: 3 / 8 ms at 200 documents (gate 150 ms at 1 000 files; the bench was not rerun: the reduction code changed only by one indexed query per alias).

**Real stack** (real embeddings of the gateway, real text recognition, `knowledge_validation_real`, model `ministral-3:3b`, no generation). Files cited with the base off AND known to hold the answer, found in the candidate set:

| Tier | Before | After | Files lost after | Reasons after |
| --- | --- | --- | --- | --- |
| 30 | 90.4 % (75 of 83), 57 questions | **93.3 %** (84 of 90), 64 questions | 6 | `not_linked_to_the_name` 6 |
| 100 | 95.6 % (173 of 181), 103 questions | **97.0 %** (191 of 197), 113 questions | 6 | `capped` 5, `not_linked_to_the_name` 1 |
| 200 | 95.3 % (224 of 235), 110 questions | **96.5 %** (248 of 257), 120 questions | 9 | `capped` 6, `not_linked_to_the_name` 3 |

**The gate (98 %) is not reached.** What remains is not the extractor's: `capped` (Q102, a practitioner's list cut at 15: batch 3), the letter that writes only `M. Dupont` (KBD-37) and the scans whose names the text recognition misread. Other real-stack figures at 200: share of cited files that really hold the answer, base off **42.5 %**, `auto` **62.3 %** (batch 1: 59.4 %); at least one right source in 174 of 183 questions against 167 with the base off; widened by themselves 37 of 120.

## Not done

- **False merges: 1 at tier 30.** A table that writes `Roy Lucas` joins the letter that writes `Lucas Roy` when no other source of the tables has yet written the pair: the cross-folder rule trades this one for the 16 duplicates. At 100 and 200 documents it is 0.
- **16 pairs `swapped_order` at 100 documents remain**, from tables that write one person in two orders inside the Data folder (`Rochat Sophie` in the appointments sheet, `Sophie Rochat` in the patients file). They are shown in « À vérifier »; the tables keep both for the engine. A detector of the order of a column (a column that holds `Dupont J` is surname first) would remove most of them: not done, proposed in the open questions.
- KBD-44 (a name split by a break in a short text), KBD-45 (`D'` and `d'`), KBD-46 (organisations no pack lists) stay `deferred`.
- The 98 % of the gate (batch 3: the cap, KBD-37, the follow-up rule). macOS.

## Documentation

`docs/DECISIONS.md`, `docs/KNOWLEDGE-BASE.md`, `defects-register.md`, `05-branches-and-stages.md`.

## Human test

`human-tests/fix-04-batch-2-names-and-organisations.md` - the figures come from the same code on the same files (the window was not opened). Status: proposed.

## Open questions

- **Swapped order inside one folder:** keep "two people with a possible match" (today), or merge by default and rely on a column-order detector (the 16 pairs would vanish, the real `Lucas Roy` / `Roy Lucas` would be one again until the detector sees a surname-first column)?
- `Helsana` (listed in the pack) and `Helsana Assurances SA` (written in the letters) are two entities: make the pack entry `Helsana Assurances SA` with the short form `Helsana`?
- The packs list 41 organisations; which ones does her library name that the dialog does not show (KBD-46)?

## Next

Batch 3, `fix/kb-retrieval-rules` (F-17, F-18, F-19, F-12, F-13, F-22, F-23, F-15), cut from `kb/integration` after this batch's merge and the tag `kb-after-fix-04-batch-2`. It must know: `CURRENT_KB_VERSION` is 3; the chunker removes blank lines; the cap of 15 still cuts people and organisations (`capped` 38 at tier 100); `open_possible_partners` takes a flag (documents ignore `swapped_order` pairs); the reduction asks for partners by side.

## Git

See the final message to the owner.
