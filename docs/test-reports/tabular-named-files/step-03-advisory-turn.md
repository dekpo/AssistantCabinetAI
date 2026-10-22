# TABULAR step 03: workbooks reach the writer

Date: 22 October 2026. Branch: `feat/workbook-cards-in-answer` (from the tip of `feat/tabular-integration`, `ce38b54`). Displays nothing new: the usage trace is data only (step 04 renders it); the catalogues are untouched.

## What changed

| File | Change |
| --- | --- |
| `apps/desktop/src-tauri/src/mixed_answer.rs` | `MIXED_INSTRUCTION` is the plan 4.4 wording (documents, workbook cards, table results; each formula on a line of its own starting with `=`). `MixedContext.cards_text`. `build_two_block_turn` becomes `build_turn(document_sources, cards, table_block)`, three blocks, `<name>: none.` for an absent one. `table_evidence_text` returns the whole block with its header `Table results (workbook "<plain path>"):`. `answer()`: a card stands for the table side of a question with no calculation (`table_unavailable` `None`), the zero-call early return needs a computed value, `InsufficientEvidence` needs no card and no documents, the reason documents were unavailable is kept after generation, the card text is known to the number checks. `verify_numbers` takes `other_known_texts`. 8 new unit tests, 3 goldens changed on purpose. |
| `apps/desktop/src-tauri/src/tabular_answer.rs` | `QuestionCards` and `question_cards(question, data, selection, locale, entity_candidates, budget_chars)`: names first (question order) then implied, one `CardRequest` per usable workbook, sheets from the selection. Pure, called by `mixed_tier` and by the tests. |
| `apps/desktop/src-tauri/src/tabular/card.rs` | `file_label(key)`: quoted plain path, plus the folder of a workbook of the other family. |
| `apps/desktop/src-tauri/src/workbook_use.rs` (new) | `WorkbookUse`, `WorkbookUseHow::{CardSent, Computed}`, `of_mixed_answer(cards, implied, table, model_wrote)`. 2 tests. |
| `apps/desktop/src-tauri/src/commands.rs` | `AskAnswer` derives `Default` and gains `workbook_use`; the nine constructors (and the test's `bare_answer`) use `..AskAnswer::default()`. `mixed_tier` builds the cards (`implied_by_entity_candidates` reads `reduction.data.candidates` minus `kept_unread`) and fills `workbook_use`. |
| `apps/desktop/src-tauri/src/lib.rs` | `pub mod workbook_use;` |
| `apps/desktop/src/lib/ipc.ts` | `AskAnswer.workbookUse`, `WorkbookUse`. |
| `apps/desktop/src-tauri/tests/mixed_routing_baseline.rs` | Cases B, C, H (last part), L and I flipped on purpose; `mixed` now builds the cards through `question_cards`; 7 new `step_03_*` tests. |
| `apps/desktop/src-tauri/tests/mixed_answer.rs`, `tests/large_test.rs` | `cards_text: ""` (the replay of `large_test.rs` sends no cards; it was **not** rerun, it needs her library and the AI machine). |
| `docs/DECISIONS.md` | One section at the end: "Taken while building TABULAR step 03". |

## Tests flipped on purpose (step 01 baseline)

- B: `baseline_case_b_the_figure_sent_to_the_model_names_its_workbook` (was `..._names_no_file`): the block is `Table results (workbook "agencies.csv"):` then the same figure line; the card of agencies.csv is in the request, factures.csv (not named, not implied) is not.
- C: `baseline_case_c_a_named_workbook_without_an_operation_reaches_the_writer_as_a_card` (English and French): one gateway call, the card with the real columns and letters, no cell value (`Harbor`, `Gamma`, `9999.75` asserted absent), `table_unavailable` `None`.
- H (last part) and L: the one assertion `table_unavailable == NotAskedAbout` became `None` and the card of the named workbook is asserted present; the `ends_with("Table results: none.")` checks stay.
- **I (not announced by the prompt):** one workbook is ticked, so rule 1 of plan 4.2 (`implied_single`) sends its card even though the question names a document; `table_unavailable` is `None`. Not an accident: the rule says so.
- Goldens of `mixed_answer.rs`: renamed `the_turn_with_documents_and_no_other_block`, `..._and_one_figure`, `the_turn_with_no_documents_and_one_figure`, now with the cards line.

## Findings

- **A formula word still goes to the yes/no sentence (case K).** The prompt's cross-file test ("a formula linking X and Y") cannot reach the writer at this step: "formula" and "formule" make `detect_structural` answer `HasFormulas`, "column(s)" makes it answer `ColumnNames`, both before any writer. The new tests and the human test use wordings without those words ("compare", "rapprocher"). Steps 05 and 07 own this.
- **Small windows.** `card_budget_chars` never goes under `CARD_SHARE_CHARS` (step 02 decision, tested), so "a tiny window shrinks the cards" of the prompt is covered by `build_cards` with an explicit small budget (`step_03_a_small_card_budget_shrinks_the_cards_and_never_drops_a_named_one`), not by the window; on a window smaller than the default the shares can exceed the prompt, as the document share already can. Left to step 16.
- **False correction avoided.** A card says "124 data rows"; a computed total is 8 412.50; without the card text in `verify_numbers` the model quoting the row count would have been "corrected" to the total. Tested both ways.
- The real model was **not** reachable (the AI machine answers ping but nothing listens on port 8080), so no model text was read by the agent; the writer path is proven with the counting fake gateway.

## Her real state (copy of her index, throwaway test deleted)

Settings: Documents folder `C:\Users\elise\AssistantCabinetAI-test\Docs` (1 253 files), Data folder `C:\Users\elise\AssistantCabinetAI\Data` (not the test Data folder), AI machine `192.168.1.83:8080`, model `ministral-3:3b`, knowledge mode `suggest`. 17 workbooks listed, 15 usable. With the document `contrats/2026-03-16_contrat-maintenance-orange.md` and the workbooks `factures-test.csv` (9 rows, `fournisseur`, `montant`, total 450), `kb-lot10-data/fournisseurs.csv` and `kb-lot10-data/reglements.csv` (3 rows each, columns `date, fournisseur, tva|acompte, echeance|reglement, montant`):

| Question | Door | Cards (reason) |
| --- | --- | --- |
| Résume factures-test.csv | 4 | factures-test.csv (named) |
| Explique-moi comment rapprocher fournisseurs.csv et reglements.csv | 4 | fournisseurs.csv, reglements.csv (named, question order) |
| Quel est le montant total de factures-test.csv ? | 3, figure 450 | - (engine alone) |
| Que peux-tu me dire de ce tableur ? (3 ticked) | 4 | the three, `implied_by_word` |
| Quelles colonnes a factures-test.csv ? | 3, structural | - |
| Que dit le contrat sur la maintenance ? (3 ticked) | 4 | none |
| same, only factures-test.csv ticked | 4 | factures-test.csv, `implied_single` |

## Measured results

- Pre-check before the first edit: `mixed_answer::tests` + `answer_shape_tests` (14), `--test mixed_answer` (22), `--test mixed_routing_baseline` (13): all green.
- Targeted runs during the work: `--lib` mixed_answer/workbook_use/answer_shape/card (40), `--test mixed_routing_baseline` (19 then 20), `--test mixed_answer` (22).
- rustfmt (`Diff in` blocks): `mixed_answer.rs` 10 → 9, `tabular_answer.rs` 48 → 48, `commands.rs` 0 → 0, `tabular/card.rs` 0 → 0, `tests/mixed_routing_baseline.rs` clean (formatted through stdin: it was clean), `tests/mixed_answer.rs` 39 → 39, `tests/large_test.rs` 2 → 2; the new `workbook_use.rs` is clean.
- Final pass, once, after the last code edit: `cargo clippy -j 2 --all-targets` (no warning in a file or a line added by this step), `cargo test -j 2 -- --test-threads=1`: 88 test binaries, 2 082 passed, 0 failed, 59 ignored; `pnpm run build` OK; `pnpm run test`: 114 files, 2 853 tests passed (the language guard covers the new Rust sources).

## Follow-up of 22 October 2026 (the owner's first human test)

Played by the owner on her real folders with `ministral-3:3b`. Verdicts: B (named workbook, no operation) conform on the facts, with one model error (« 9 lignes dont 1 en-tête ») and an invalid formula (`SOMMEIFS`, a case for step 07); C (engine) conform; D1 (« ce tableur », three ticked) **not** conform to the expectation: the unrelated contract excerpt took over; D2, E3 conform; E2 conform with a false « 10 » warning. Three changes were made on the spot, as she asked:

1. **Sources.** A workbook whose card was sent is listed among the sources (`workbookCards`, `sourceLines`, `MessageList.tsx`, key `chat.sourceItemCard` in both catalogues, one vitest). It is copied with the other sources when `copySources` is on (it is on in her settings).
2. **Card wording** (`card.rs`): `9 data rows, on rows 2 to 10; header on row 1, not counted`. Five card tests updated.
3. **D1 measured on the real turn, three shapes, two models** (throwaway harness, deleted): the current turn; cards first plus a sentence « answer the question asked, ignore a source that does not bear on it »; no document block. `ministral-3:3b`: current turn describes the workbooks but mixes the contract in; the sentence version answers « no information about a spreadsheet » (worse, rejected); the no-document version is clean. `gemma4:e4b`: all variants correct and complete, 6 to 12 s after a 94 s cold load. Conclusion recorded in `DECISIONS.md` row 14: the fix for a small model is to leave the excerpts out of an advisory question about a workbook (step 05), not a sentence.

## Second human test (22 October 2026), checked line by line

Played by the owner after the three changes above (script `step-03-correctifs-test-humain.md`). Her questions were the ones of the script; every answer was checked against the files (`factures-test.csv`: `fournisseur`, `montant`, 9 rows, total 450; `fournisseurs.csv`: `date, fournisseur, tva, echeance, montant`; `reglements.csv`: `date, fournisseur, acompte, reglement, montant`; 3 rows each).

| Part | Result | Verdict |
| --- | --- | --- |
| A1 `Résume factures-test.csv` (ministral) | columns A/B right, « 9 lignes (lignes 2 à 10), sans en-tête comptée », no invented figure; Sources: the contract and `factures-test.csv (structure seulement ...)`, also in the copy | conform; the model's header error of the first test is gone |
| A3 `rapprocher fournisseurs.csv et reglements.csv` (ministral) | common column B right, D `reglement` and E `montant` right in step 3; **the closing remark swaps the letters** (« D (accompte) et C (règlement ») and writes `accompte`; the formula `=SI(RECHERCHEV(B2; reglements!B:A; 2; FAUX); reglements!D2; "")` is invalid (reversed range, a file used as a sheet, a lookup result used as a condition) and « l'échéance » is a column of the other file | facts mostly right, formula and one letter pair wrong: **real material for step 07** |
| A4 total of `factures-test.csv` | 450, 9 rows, engine alone, **no** « structure seulement » line | conform |
| B1 `Que peux-tu me dire de ce tableur ?` (two ticked: contract, `factures-test.csv`) | structure right (« lignes 2 à 10 »), **no** warning about « 10 » | conform; the sentence « pas de noms de colonnes dans les extraits » beside the columns it just listed is clumsy (model) |
| C2 same question, `gemma4:e4b` (three ticked) | the three workbooks, every letter and row range exact, no contract, 15 s | conform, as measured |
| C3 `rapprocher ...`, `gemma4:e4b` | right common column (B in both files), nothing wrong, **no formula** (terse) | conform on facts; not a defect |
| C2 and C3, `granite3.1-moe:3b` | **fails**: says « pas de résultats de table », lists the contract, gives `fournisseurs.csv` columns A to C only and omits `factures-test.csv`; then invents « colonnes E et F », a column F that does not exist, and « taux d'imposition » for C and D | not usable for this task (already known small MoE): evidence for step 25 |

**Defect found and fixed:** two « Sources » disclosures under one answer (documents, then workbooks): merged into one (`sourceItems`), vitest added. Row 15 of the DECISIONS section.

**Candidates proposed for the next comparison** (licence rows written in `models/LICENSES.md`, nothing pulled): `qwen3:4b-instruct` (Apache 2.0, 2.5 GB, non-thinking) and `qwen2.5-coder:7b` (Apache 2.0, 4.7 GB; the 3B Coder is under a research licence and is out).

Final pass rerun after these edits: `pnpm run build` OK, `pnpm run test` 114 files, 2 855 tests.

## Third human test (22 October 2026): `qwen3:4b-instruct`, checked

The owner added `qwen3:4b-instruct` on the tower (`MODEL_ALIASES` line, backup `.env.avant-qwen3`) and replayed the questions **in one conversation without clearing it** (history carried between questions, and the last part with one workbook ticked came last, which is why the order of the script was wrong: see the feedback below). Her pasted answers, checked against the files:

| Question | Answer | Verdict |
| --- | --- | --- |
| `Résume factures-test.csv` | « Je ne peux pas résumer: aucun extrait ni résultat de table ne fournit les données » | **Not conform**: the card gives the structure; refusal |
| `rapprocher fournisseurs.csv et reglements.csv` | « Je ne peux pas expliquer ... aucune donnée » (it even says the structure is provided) | **Not conform**: no method, no join column |
| total of `factures-test.csv` | 450, engine alone | conform |
| `Que peux-tu me dire de ce tableur ?` (three ticked, history of refusals) | the three workbooks right, letters and row ranges exact, but « trois feuilles » (they are files), and it adds « pour le montant total ... aucune donnée » (a leak of the earlier question, contradicting the 450 shown above) | facts right, noise from the history |
| same, one workbook ticked | structure right (header row 1, rows 2 to 10); « une feuille nommée factures-test.csv » (a CSV has none) and `Extrait : [1]` citing the contract for nothing | facts right, one bogus citation |

**Cause and fix (row 16 of DECISIONS):** the instruction says « never invent a value » and the cards say « no data values »; a literal model (`qwen3:4b-instruct` follows instructions closely) concluded that a summary or a reconciliation needs data. Measured on the real turn, 3 questions × 2 wordings × 3 models: adding one sentence (a card is a real source about structure, do not refuse a structure question for lack of data) turns the two refusals into correct answers (`qwen3:4b-instruct`), changes nothing for `gemma4:e4b`, and `ministral-3:3b` stays correct with its usual variance. Applied to `MIXED_INSTRUCTION`; guard test added.

**Left as it is, with the owner informed:** the bogus `[1]` citation and the contract listed in Sources when the answer does not use it (an excerpt is sent and cited though irrelevant: step 05 skips the excerpts for advisory questions); `=JOIN(fournisseurs.csv!B2:B4, reglements.csv!B2:B4)` proposed by `qwen3:4b-instruct` is not a spreadsheet function (a case for step 07, added to its prompt); « feuilles » for files (model wording).

Final pass after the instruction change: Rust 88 binaries, 2 082 passed, 0 failed (front unchanged).

## Fourth human test (22 October 2026) and the data-sample question

Same questions with `qwen3:4b-instruct`, application relaunched with the structure sentence. Still: `Résume factures-test.csv` refused (« impossible de résumer sans données »), `rapprocher` answered with the right join column (B) but two meaningless formulas (`=SI(A2=B2; C2; "")`), « ce tableur » right on structure but followed by seven lines of « impossible de rapprocher sans données » (the conversation was not cleared, so the previous turn fed the loop). The owner concluded that models need at least a sample of the cells and asked for the risk analysis.

**Measured on the real turn (throwaway harness, deleted; 6 runs per cell at temperature 0.1, the refusal count is a keyword count, indicative):**

| Model | Question | Structure only | With a 5-row sample (all rows when the file has 3) |
| --- | --- | --- | --- |
| `qwen3:4b-instruct` | Résume factures-test.csv | **refused 4/6** | refused 0/6 |
| `qwen3:4b-instruct` | rapprocher fournisseurs / reglements | refused 0/6 (the sentence of row 16 works here) | refused 0/6, and it notices that the amounts of both files agree row by row |
| `ministral-3:3b` | Résume factures-test.csv | refused 0/6 | refused 0/6, **but 1/6 invented partial totals** (« Alpha : 10 + 40 = 50 », when the table holds 120: it summed the five rows it saw) |
| `gemma4:e4b` | all three questions | correct | correct, adds example values |

So: a sample removes the refusals of a literal model on a summary (the owner's hypothesis holds for that task), and it creates a new failure, a total computed from a partial sample and presented as fact. The design below answers both. **Not built, not decided: awaiting the owner's choices** (see the report of 22 October 2026): a `Table sample` block after the cards, the whole table when it fits the budget and otherwise a labelled head sample sized from the model's budget; cells cleaned and « data, not instructions » stated; identifier-like values masked by the packs; shown in Sources and in the trace as rows sent; a selector in the settings (structure only / structure and sample); the engine stays the only source of figures. It is the base of step 10 (rows chosen by the engine), which extends it with rows chosen by filters.


## Corrections made to later prompts

- `04-usage-trace`: new block "Names you inherit from step 03" (module `workbook_use`, the helper to extend, `question_cards`, what has no entry yet, `table_unavailable` already `None` with a card, case I).
- `05-tables-only-advisory`: new block "What step 03 delivered that this step uses" (the `answer()` rules with cards, `InsufficientEvidence` to catch, `documents_unavailable` kept, the words that reach the engine first, which frozen tests already flipped).
- `06-explain-and-analyse-calculation`: new block (`table_evidence_text` is the one place and its header, `verify_numbers` parameter, `workbook_use` module, the zero-call rule).
- `07-checked-formula-help`: `verify_numbers` parameter. `08-named-file-wins`: case H expectation (`table_unavailable` `None`, the card present). `10-rows-chosen-by-engine`: `build_turn`, rows share in the window test. `11-follow-up-memory`: `NotAskedAbout` becomes `None` with a card.
- `00-PLAN`: section 1.2 (pointer to `build_turn`), case C verdict, section 1.4 (`MixedContext` built in four places), a paragraph "As built by step 03" before 4.5.
- Read, nothing to change: `09`, `12` (its status line already lists the `workbook_use` shape as something to consolidate).
