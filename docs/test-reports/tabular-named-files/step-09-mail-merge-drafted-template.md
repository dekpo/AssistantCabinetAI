# TABULAR step 09 - Mail merge without a template

Branch `feat/mail-merge-drafted-template`, from `feat/tabular-integration` after step 08. 23 October 2026. The decisions are in `docs/DECISIONS.md`, "Taken while building TABULAR step 09 (23 October 2026), to confirm"; this file is the record of what changed and what was measured.

## What changed

- **New module `fill_draft.rs`** (Tauri-free): `TEMPLATE_DRAFT_INSTRUCTION` (profession-neutral, guard test), `fields_of` (a placeholder name for every column, unique, within the placeholder alphabet), `resolve` (the program's table first, then `propose_mapping`), `clean_draft` (reasoning tags, fence, commentary line, 20 000 bytes), `draft_turn`, `draft` (one gateway call, one retry on an answer with no placeholder, `DraftFailure::{Cancelled, Unreachable, Unusable}`).
- **`fill_plan.rs`**: `FillDecision::NeedsDraft(FillSource)` (was `Ask(FillNeedsTemplate)` for a named workbook with no fitting template), `draft_decision` (the call and the plan or the honest answer), `column_kinds`, `build_drafted_plan`, `TemplateFile::drafted` / `proposed_mapping`, `FillPlan.template_origin` / `draft_text`, `FillRequest.draft_text`, `FillReport.model_file`, `write_model` (the approved text as `<stem>-letter-model.txt`, first, never overwriting), `"drafted": true` in the log line.
- **`commands.rs`**: `FillStart::{None, Answer, NeedsDraft(Box<DraftJob>)}` as the return of `fill_tier`; `fill_draft_tier` awaits the draft in `sourced_answer`; `fill_inputs` builds the in-memory template from `draft_text` and re-validates it.
- **`TabularAnswer::FillNeedsTemplate`** gains `draft` (`unreachable` | `unusable`, absent otherwise); `AppError::FillDraftTooLarge` (`fill_draft_too_large`).
- **Interface**: `FillPlanCard` shows the drafted template in an editable text area (preview after a 400 ms pause, decisions merged by `lib/fill.ts::mergeDecisions`), « Rédiger à nouveau » (`onResend`), the saved-model line; `useChat` and `annotations.ts` word the drafted plan; both catalogues (`fill.introDrafted`, `draftTitle`, `draftNote`, `draftTooLarge`, `draftAgain`, `modelSaved`, `tabularAnswer.fillDraftFailed.*`, `errors.fill_draft_too_large`).
- **Tests updated for the new decision**: `tests/named_file_wins.rs`, `tests/mixed_routing_baseline.rs` (`NeedsDraft` instead of `FillNeedsTemplate`), `tests/large_test.rs` (the replay harness stops at `fill_draft`, it has no model), `tests/fill_plan.rs` (literal), `workbook_use.rs` (pattern and test literal).

## Tests

- `tests/fill_draft.rs` (9 tests, run alone with `--test-threads=1`): drafted plan and nothing written; **no cell value of the workbook is in the request** (distinctive values asserted absent, field names asserted present); altered and unknown placeholders (proposed / unresolved and shown); fence and commentary cleaned; one retry on no placeholder (the second request is stricter); twice no placeholder = `FillNeedsTemplate { draft: "unusable" }`, zero files; a server that hangs up = `unreachable`, zero files; a body that is not a stream is no plan; approval writes the letters and the model, a second approval writes `-2` names and overwrites nothing, the log holds no cell and no text; an edited draft previews, a tampered one is refused (size, blank) or left unresolved (a column the table lacks).
- Unit tests in `fill_draft.rs` (7) and `lib/fill.test.ts` (4 new), `tabularAnswer.test.ts` (the failed-draft sentences).
- Pre-check before the first edit: `fill_plan` unit tests (6), `tests/fill_plan.rs` (5), `tests/named_file_wins.rs` (10), `tests/publipostage.rs` (10), `tests/template_fill.rs` (6): green.

## Final pass (once, after the last edit)

`cargo test -j 2 -- --test-threads=1`: exit 0, 2251 passed, 59 ignored, 0 failed (94 result lines). `cargo clippy --all-targets`: no warning in a file of this step (the first run flagged `FillStart::Answer` as a large variant; boxed, then `clippy --lib` clean). `pnpm run build`: ok. `pnpm run test`: 116 files, 2925 tests passed (the source guard included). The boxing of `FillStart::Answer` came after the full Rust run; it is a type change checked by `clippy --lib`.

## Measured (her real state, a copy of her index, her tower)

Data folder `C:\Users\elise\AssistantCabinetAI\Data`, documents `C:\Users\elise\AssistantCabinetAI-test\Docs`, everything ticked. Through the real path (`locate_fill_source`, `decide`, `draft_decision`):

- `factures-test.csv` (columns `fournisseur`, `montant`; 9 rows): no template fits among all 1253 documents -> `NeedsDraft`; draft in 1.4 to 3.6 s with `qwen3:1.7b`; the first letter is filled with Alpha / 10.
- `Test/factures-fournisseurs-2026.xlsx` (8 rows): the template `modeles/modele-relance-facture.txt` fits -> file plan, 8 letters, no draft. Without that one document, nothing else fits `date, fournisseur, montant`.
- A gateway at `127.0.0.1:9`: `FillNeedsTemplate { draft: "unreachable" }` in about 2 s.
- **Quality**: in 3 of 3 runs `qwen3:1.7b` invented extra placeholders (`societe`, `adresse`, `nom_societe`, `email`, ...) despite « and no other placeholder », and once repeated its lines; they stay unresolved and listed. No invented value reached a letter. A larger model was not measured (step 25).

## Corrections made to the next prompts

- `SESSION-TABULAR-10-rows-chosen-by-engine.md`: the merge-request rule now says a named workbook with no template is `NeedsDraft` (drafted plan, or `FillNeedsTemplate { draft }`), and names the test that asserts that no row is in the request of the draft.
- `SESSION-TABULAR-12-wave1-closure.md`: the drafted-template topic lists the final names and the `.txt` outcome.
- `SESSION-TABULAR-00-PLAN.md`: case H row (step 09), section 8 point 4 (where the approved template is saved, and the two questions left to the owner).
- `docs/DECISIONS.md`, step 08 row 7, marked superseded.
- Prompts 11 and later were not read: the plan table gives them no dependency on this step (only 10 and 12 follow it in the chain).

## Left for the owner

Word output of a drafted template; drafting for an implied workbook (N-24); a letter request with only tables ticked (N-25); the grouping hint (not built, no answer to the opening note).

## Owner's human test (23 October 2026, model `ministral-3:3b`, everything ticked by « Select. auto »)

| Part | Result |
|---|---|
| 0 | Backups made, no `Generated`, fingerprints noted: as scripted. |
| 1 | **Conforms.** The sentence, the card with the editable box, the note « rédigé par l'IA à partir du nom des colonnes seulement », the fields `fournisseur -> fournisseur`, `montant -> montant`, the preview with Alpha / 10, « 9 courrier(s) ... factures-test-letter-1.txt », the trace line, and `Generated` absent after the question. The draft (ministral) used only the two column fields; it also wrote two bracketed lines: `[Coordonnees si necessaire]` is read as a field (unresolved, listed, left as written), `[Nom de l'entreprise ...]` is not (an apostrophe is not in the placeholder alphabet) and stays plain text. It added the unit « euros » that the column does not state, and some odd phrasing (« facture emis(e) sous le numero de fournisseur »): text of the model, not data. |
| 2 | Conforms (the preview followed the first line she typed, « Objet : Relance de paiement »). |
| 3 | Conforms: 10 files (`factures-test-letter-1..9.txt` and `factures-test-letter-model.txt`); letter 1 md5 `75e2b82a47f8734538a5d334d1b15f8f`. Checked afterwards by reading the files: every letter carries the typed first line and its own supplier and amount (Alpha 10 ... Gamma 90); the saved model holds the text as approved with `{{fournisseur}}` and `{{montant}}`; the journal has 10 lines with `"drafted": true`, an empty template and no cell. |
| 4 | **Differs from the script (N-29).** The same question found the **saved model** as a template (« J'ai trouvé le modele Generated/factures-test-letter-model.txt ») and made a file plan, no new draft. She generated: 9 new files `factures-test-letter-model-1..9.txt` (19 files in all, no new model file); letter 1 unchanged (same md5): nothing overwritten. |
| 5 | **Differs from the script (N-29).** For `Test/factures-fournisseurs-2026.xlsx` the card proposed the saved model, not `modeles/modele-relance-facture.txt` (8 letters). Not generated. |
| 6 | **Not played as written** (N-30): the question gave the saved model's plan again. |

Files left in place for inspection until she says to restore: `Generated` (19 files), the settings and journal backups `*.avant-tabular-09`.

## After the owner's test

- **Part 6bis (unreachable AI, after deleting `Generated`): conforms.** The sentence « La machine IA n'a pas pu etre jointe ... » then the step 08 sentence, no card, nothing created. Restoration done by the owner (fingerprints of the template, the workbook and the csv identical to part 0).
- **Fix of N-29, validated by the owner and built:** `fill_plan::without_output_folder` removes what the program wrote from the template search. Targeted checks: the new unit test, `cargo test --test fill_draft -- --test-threads=1` (10 of 10, one added), `vitest` guards and catalogues, `clippy --lib` clean, `rustfmt` blocks unchanged (`fill_plan.rs` 24, `commands.rs` 0). The full suite was not run again for this small change.
- Not changed, to note: the letters made from a copied model are named after its stem (`<model stem>-<key>.txt`).
