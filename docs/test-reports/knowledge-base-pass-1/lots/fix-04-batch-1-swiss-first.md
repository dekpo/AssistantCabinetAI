# Fix 4, batch 1 - What a Swiss practice meets on day one (F-00, F-01, F-02, F-03, F-11, F-14, F-16, F-24, F-20 line)

12 October 2026 · branch `fix/kb-swiss-first` · based on `kb-after-lot-11` (`2f0700e`, lot 11 merged, PR #36) · tab "KB fix 4 - Battery (Swiss first)"

Written for the developer or agent of the next batches, and for the owner. English; French sentences are quoted test data.

## Summary

Nine fixes of the battery, each proved by a test that was written first and seen failing. A Swiss woman doctor (`Dre`) is found under her own name; an AVS number, a policy number, an e-mail address or an acronym such as `HUG` is no longer taken for a file name; a commune written in a table no longer narrows the documents; `CHF 1'450.00` is a number to the table engine; a workbook named by hand (`factures-fournisseurs-2026.xlsx`) is counted whole; a column called `Total_Ligne` is summed; the search over many explicit files is indexed; the Analyse summary says which files it could not read. On the sandbox at 200 documents the recall of the files that hold the answer, among the questions the base reduces, goes from **85.9 % to 96.3 %**, the wrong-entity questions from 3 to 2, the questions the folder router wrongly answers from 11 to 3, and the files lost to the cap of 15 from 33 to 0.

## What changed

**Rust, `apps/desktop/src-tauri/src/`**

- `knowledge/mod.rs` - `CURRENT_KB_VERSION` 1 to 2, with the history in its comment.
- `resources/knowledge/base/fr-FR.json` - titles `Dre`, `Dresse`, `Docteure`, `Professeure`; weak title `Pre`; their spoken forms. Data only.
- `knowledge/resolve.rs` - in a question, a title and a surname that the base has seen together (`dre vuagniaux`) outrank the surname alone, before the generic alias lookup. Ingestion unchanged.
- `knowledge/reduce.rs` - `only_orders_documents`: a `location` entity does not drive the reduction of the documents (a chip still does; the tables still reduce on a place).
- `file_reference.rs` - `looks_like_a_file_name`: the extension must be 1 to 5 letters or digits with a letter in it; a token with `@` is not a file name.
- `folder_questions.rs` - `acronyms_in`: an unknown word of 2 to 6 capitals, in a sentence that has lowercase letters, counts as what the question is about (the corpus test cannot see words under four letters).
- `tabular/inventory.rs` - `CHF`, `Fr.`, `SFr.` as unit symbols (any case, longest first); `'` and `’` group thousands and are never a decimal mark; the sign may follow the currency.
- `tabular/question.rs` - `without_the_run`; `detect_structural` reads the question without the words of a multi-word column named whole; `classify_with` takes the column name out of the operation words as one run (so `le total de Total_Ligne` keeps its operation).
- `tabular_answer.rs` - `pick_target` also says whether the workbook was named; `without_the_workbook_reference` takes its path, file name or hyphenated name out of the text the reader sees.
- `index_store.rs` - `CREATE INDEX IF NOT EXISTS chunks_by_document ON chunks (relative_path)`.
- `indexing.rs`, `data_folder.rs` - `IndexSummary.unread_formats` (`UnreadFormat { extension, count }`); the documents pass fills it, the Data Folder pass leaves it empty.

**Client, `apps/desktop/src/`** - `lib/unreadFormats.ts` (+ test), `lib/ipc.ts` type, `WorkFolderCard.tsx` one line, `locales/fr-FR.json` and `en-US.json` keys `workFolder.unreadOne/Many/Format`.

**CI** - `.github/workflows/ci.yml`: second step `cargo test --tests`.

**Tests** - `tests/knowledge_fix_swiss_first.rs` (16 tests, 3 ignored prints), `tests/publipostage.rs` (+2), `src/lib/unreadFormats.test.ts` (4).

**Documentation** - `docs/DECISIONS.md` ("Settled by KB fix 4, batch 1"), `docs/ARCHITECTURE.md` (units), `docs/KNOWLEDGE-BASE.md`, `defects-register.md` (7 rows fixed, KBD-39 and KBD-40 added, notes).

**Tooling** - `human-tests/lot11_helper.py`: command `swiss-file`; `human-tests/fix-04-batch-1-swiss-first.md`.

## Decisions taken

| Decision | Reason | Where recorded |
| --- | --- | --- |
| `Pre` is a **weak** title, not a strong one as the battery listed | Folded, `Pre` is the word `pre` (« pré »); a weak title needs a capital and a capitalised word after it | DECISIONS |
| The title-and-surname rule is added (not in the battery text) | Without it, `Que dit la Dre Vuagniaux ?` became an ambiguity with the patient Gilles Vuagniaux once `Dre` stopped being part of the name; the battery's acceptance is that it gives the same files as the full name | DECISIONS |
| A bare word that is a workbook's name is not taken out of the question (`montants` for `montants.csv`) | It may be the column. Only the path, the file name with its extension, or a hyphenated name are references | DECISIONS |
| Acronym rule in the folder router, not in the file reference | Reading the code showed the `HUG` listing came from `folder_answer` (unknown words under the limit), not from `FileReferenceResolver` as the register guessed | register KBD-35 note |
| An e-mail address is not a file name | Same family, one condition, same proof | register note |
| F-03: the location rule, not the cap rule | The cap still protects against a long lexical-only list for people; the trigger of the harm was the place | DECISIONS |

## Deviations

- The battery says for F-01 « all-but-one at 878 files falls from a median 320 ms to under 30 ms ». It falls to **48 ms** (p95 137 ms), a factor of 6.6. The remaining cost is the one query per file; a single `IN` query would remove it (not done, not needed for the gate: batch 3 candidate).
- F-02's acceptance (`Dre Vuagniaux` and `Anne-Sophie Vuagniaux` give the same files) needed the resolver rule above. It was not in the battery.
- F-20's acceptance « the 21 files of 200 are counted » is met in total (21); by extension the sandbox gives 7 `.odt`, 5 `.rtf`, 4 `.eml`, 2 `.vtt`, 1 `.odp`, 1 `.ppt`, 1 `.pptx` (the register's split 7/7/2 was approximate).

## Invariants

I1 to I4 and the others: unchanged suites green (`knowledge_isolation`, `knowledge_targeting`, `knowledge_validation`: 0 file outside the selection, I3 snapshot equal). Two behaviours changed on purpose and are proved: a place never drives the documents; a question names a person by title first. `reduced_scope_is_always_a_subset_of_the_selection` unchanged.

## Verification (Windows, this PC)

| Command | Result | Before the batch |
| --- | --- | --- |
| `cargo test --lib --tests --no-fail-fast` | **1 269 passed**, 0 failed | 1 248 |
| `pnpm run test` | **627 passed** | 622 |
| `pnpm run build` | OK | OK |
| `cargo clippy --lib` | 21 findings | 21 |
| `rustfmt --edition 2021 --check` on touched files | no new finding (counted per file against `HEAD`); the new test file is formatted | |
| macOS | **not run** (Mac mini not delivered). CI runs the new step on both runners on the pull request: read both before trusting it | |

Proofs, fix by fix (all in `tests/knowledge_fix_swiss_first.rs` unless noted):

| Fix | Failed first with | Passes now |
| --- | --- | --- |
| F-02 | `no entity named "Anne-Sophie Vuagniaux" among ["Dre Anne-Sophie Vuagniaux", "Dre Sandra Pasquier", "Dresse Sandra Pasquier"]`; the shared-surname question was an ambiguity | `a_swiss_woman_doctor_is_found_under_her_own_name`, `the_question_with_and_without_the_given_name_keeps_the_same_files`, `a_title_and_a_surname_seen_together_outrank_the_surname_alone` |
| F-03 | `"Quels documents viennent de Genève ?": the documents were reduced to [3 files]` | `a_commune_of_a_table_never_narrows_the_documents` |
| F-16 | `"À qui appartient le numéro AVS 756.3052.5778.43 ?" -> Deterministic(NoMatchingFile ...)` | `an_identifier_or_an_acronym_goes_to_the_documents_not_to_the_file_router`; guards `a_real_file_name_still_names_its_file`, `the_questions_about_the_folder_are_still_answered_from_the_folder` |
| F-24 | the engine « handed the question to the model »; `NonNumericColumn` | `swiss_amounts_are_computed_by_the_engine_like_the_french_ones` (8 941,54 and 2 310,25 in both files), `every_swiss_spelling_of_an_amount_is_one_number`, `an_apostrophe_groups_thousands_and_is_never_a_decimal_mark` |
| F-14 | `Count(0)` with filter `N° facture = 2026` on the sandbox's invoices workbook; `a_year...is_still_a_filter` read 5 for 3 | `the_name_of_a_workbook_is_a_reference_not_a_date`, `the_sandbox_invoices_workbook_named_by_hand_is_counted_whole`, `a_year_the_user_really_asks_for_is_still_a_filter` |
| F-11 | `Structural RowCount 5` for `la somme de Total_Ligne` (`publipostage.rs`) | `a_column_named_with_a_rows_word_is_computed_not_counted` (1 592,98 / 899 / 318,596), `a_plain_row_count_is_still_a_row_count` |
| F-01 | only `sqlite_autoindex_chunks_1` | `the_chunks_of_one_document_are_found_through_an_index` |
| F-20 line | compile error (field absent); the vitest file is new | `the_analyse_summary_counts_the_files_it_could_not_read_by_format`, `a_folder_with_only_readable_files_reports_no_unread_format`, `unreadFormats.test.ts` |

## Measured - before / after

Machine: this PC (Windows), release build, fake vectors for the replay (`knowledge_validation`), sandbox `fixtures/knowledge-sandbox`. Before: `measurements/quality-tier-N.md` (lot 11). After: `measurements/fix-04-batch-1/`. Mode `Auto`, structural replay of 127 / 195 / 205 questions.

| Figure | Tier 30 before | after | Tier 100 before | after | Tier 200 before | after |
| --- | --- | --- | --- | --- | --- | --- |
| Questions whose documents were reduced | 63 | 61 | 109 | 108 | 115 | 112 |
| **Recall of the reduced set** (files that hold the answer) | 90.2 % (92/102) | 88.5 % (77/87) | 82.3 % (265/322) | 82.5 % (227/275) | 85.9 % (353/411) | **96.3 % (312/324)** |
| Wrong entity picked | 1 | **0** | 3 | 2 | 3 | 2 |
| Leaks (file outside the selection) | 0 | 0 | 0 | 0 | 0 | 0 |
| Routed to the folder router (not reduced) | 10 | 3 | 11 | 3 | 11 | 3 |
| Relevant files lost, `capped` | 0 | 0 | 46 | 38 | 33 | **0** |
| Relevant files lost, `no_mention_in_file` | 0 | 0 | 1 | 0 | 11 | 0 |
| Relevant files lost, `weak_form_only` / `carry_over` | 8 / 2 | 8 / 2 | 7 / 3 | 7 / 3 | 7 / 7 | 5 / 7 |
| Persons of the documents found | 25 / 26 | 26 / 26 | 72 / 78 | 76 / 78 | 92 / 104 | 96 / 104 |
| `title_full` link recall (person) | | | | | 92.7 % | 99.4 % |
| Names of the documents never found | 8 | 7 | 18 | 14 | 29 | 25 |
| False merges (`Lucas Roy` / `Roy Lucas`) | 2 | 2 | 2 | 2 | 2 | 2 |
| Category `practitioner`, recall when reduced | | | | | 77.4 % | **100 %** |
| Category `organization`, recall when reduced | | | | | 92.0 % | 100 % |
| Category `identifier`, answered by the folder router | | | | | 7 | **0** |

Why fewer questions are reduced (`no_discriminating_entity` 2 to 8, `no_entity` 39 to 43 at 200): the place questions (`Quels documents mentionnent Annecy ?`) are no longer narrowed on a commune. They search the whole selection, which is the intended reading; the gate counts recall, not the share of reduced questions.

Search over an explicit selection of all-but-one (`measurements/fix-04-batch-1/bench.md`, `Off`, ms median / p95): 500 files 80.5 / 137.1 to **19.7 / 43.5**; 878 files **320.1 / 526.2 to 48.3 / 136.8**. Lookup p95: 19 to 26 ms at 878 files (gate 150), 13 ms at 89 files (gate 30).

Unread formats by tier, from the product's Analyse summary: tier 30 `2 (1 .odt, 1 .vtt)`, tier 100 `8`, tier 200 `21 (7 .odt, 5 .rtf, 4 .eml, 2 .vtt, 1 .odp, 1 .ppt, 1 .pptx)`.

Real stack (real embeddings of the gateway, real OCR, `knowledge_validation_real`, `ACAI_TIERS=30,100,200`, model `ministral-3:3b`, no generation): see the section « Real stack » below.

## Real stack - recall of the files that hold the answer (the gate figure)

Files cited with the base off AND known to hold the answer, found in the candidate set (`measurements/fix-04-batch-1/real-tier-N.md` against `measurements/real-tier-N.md`):

| Tier | Before | After | Files lost before | after | Reasons after |
| --- | --- | --- | --- | --- | --- |
| 30 | 91.6 % (87 of 95), 58 questions | **90.4 %** (75 of 83), 57 questions | 8 | 8 | `not_linked_to_the_name` 8 |
| 100 | 95.9 % (186 of 194), 103 questions | **95.6 %** (173 of 181), 103 questions | 8 | 8 | `capped` 5, `not_linked_to_the_name` 3 |
| 200 | 91.5 % (237 of 259), 112 questions | **95.3 %** (224 of 235), 110 questions | 22 | 11 | `capped` 6, `not_linked_to_the_name` 5 |

Read honestly: **the gate (98 %) is not reached**, and at tiers 30 and 100 the figure did not improve, it moved by the denominator. The files lost there are exactly the same as before, file by file: the scan of `Meunier Veronique` (OCR damaged the name) and the letter that writes only `M. Dupont` (KBD-37, batch 3); at 100 also Q102, a practitioner whose list the cap of 15 cuts (5 files). The loss that this batch removes is at 200 documents: the three `Dre` sentences (Q092, Q093, Q202) and the hospital question Q103 (`not_linked_to_the_name` 15 to 5, `capped` 7 to 6, 22 files lost to 11). What remains at 200 is Q102 (six files cut by the cap), the scans of three letters and the `M. Dupont` letter.
Other real-stack figures at 200: share of cited files that really hold the answer, base off **42.5 %**, `auto` **59.4 %** (before 43.0 % and 59.9 %); at least one right source in 172 of 183 questions in `auto` against 167 with the base off; widened by themselves 27 of 110.
The two Dre sentences on the real stack: `Que dit la Dre Vuagniaux ?` and `Quels courriers a écrit la Dre Anne-Sophie Vuagniaux ?` reduce to the same 6 files at 200 (3 at 100). The first one is then **widened** by the product (« peu de résultats »), the second is not: the widening rule, KBD-17, batch 3.

## The owner's replay (12 October 2026) and what it changed

Every step of Parts B and C matched except the lines below; Part A was not reported. Found and fixed in the branch: **KBD-42** (the targeting line wrote the internal label `health_insurance_number da99a7` as a name: an identifier is never written as a name now), **KBD-43** (false `Le chiffre « 03 » ne figure dans aucun des extraits` on the month of a cited file name: the cited paths are known to the number check), **KBD-39** (« la somme de la colonne X » listed the columns: the word « colonne » now lists them only when the question holds no sum, mean, median, minimum or maximum). Found and deferred: **KBD-41** (the sandbox's hand-written `.xls` reads with a NUL after every letter and its dates as numbers, which produced the garbled suggestion of her screenshot; a real Excel `.xls` is to be tried). Not a defect of the batch, seen again: `Que dit la Dre Vuagniaux ?` was widened by the product (KBD-17) and the AI's answer then described another patient. After these fixes: 1 269 cargo, 627 vitest, build OK.

## Not done

- The gate figure (98 % at 100 and 200 on the real stack) is not reached by this batch alone: what remains at tier 200 is `weak_form_only` 5 and `carry_over` 7 (batch 3), the organisations the extractor does not read (25 names, batch 2) and the apostrophes (Q054, Q081, batch 2).
- The cap of 15 still applies to person and organisation reductions: at tier 100 `capped` is still 38.
- KBD-18 (a person's name that is also a file's stem) and the extractors of KBD-30 stay `deferred`.
- A sandbox case with `CHF` amounts was not added to the generator: the Swiss amounts are proved with the probe's own table (in the test and in the helper's `swiss-file`). Proposed for the last batch, append-only.
- macOS.

## Documentation

`docs/DECISIONS.md`, `docs/ARCHITECTURE.md`, `docs/KNOWLEDGE-BASE.md`, `defects-register.md`.

## Human test

`human-tests/fix-04-batch-1-swiss-first.md` - walked on temporary copies by the agent (the window itself was not opened). Status: proposed.

## Open questions

- `Pre` as a weak title: confirm, or make it strong.
- KBD-39 (« la somme de la colonne X » lists the columns): batch 3 or leave?

## Next

Batch 2, `fix/kb-names-and-organisations` (F-06, F-07, F-08, F-09, F-10, F-04), cut from `kb/integration` after this batch's merge. It must know: `CURRENT_KB_VERSION` is 2 (raise it again if the extractor changes); the base titles live in `resources/knowledge/base/fr-FR.json`; the resolver now prefers a stored title-and-surname form in a question; the sandbox's remaining misses are the 25 names of the table above.

## Git

See the final message to the owner.
