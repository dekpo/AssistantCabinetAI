# TABULAR step 02: workbook cards and naming

Date: 22 October 2026. Branch: `feat/workbook-cards` (from the tip of `feat/tabular-integration`, `e8cc612`). No routing changed, nothing displayed, no catalogue touched.

## What changed

| File | Change |
| --- | --- |
| `apps/desktop/src-tauri/src/tabular/card.rs` (new) | The workbook card: `column_letter`, `CardOptions`, `WorkbookCard`, `build_card`, `join_cards`, `build_cards` (+ `CardRequest`, `CardBlock`), `CARDS_HEADER`, `CARD_SHARE_CHARS` (4 000), `card_budget_chars`. Pure, no `tauri::` import, structure only. 19 tests. |
| `apps/desktop/src-tauri/src/tabular/mod.rs` | `pub mod card;` |
| `apps/desktop/src-tauri/src/file_reference.rs` | `FileReferenceResolver::resolve_all_in_question` and `PositionedResolution`; helpers `tokens_with_offsets`, `word_spans`, `quoted_phrases`; 9 tests. `resolve_in_question` unchanged. |
| `apps/desktop/src-tauri/src/tabular_answer.rs` | `NamedWorkbooks`, `named_workbooks`, `ImpliedReason`, `ImpliedWorkbook`, `implied_workbooks`, `MAX_IMPLIED_WORKBOOKS`; `names_a_workbook` now built on `resolve_all_in_question`; the "in the folder but not selected" lookup of `pick_target` moved to a shared `workbook_elsewhere` (same behaviour); 15 tests. |
| `apps/desktop/src-tauri/src/tabular/question.rs` | `PatternPack.table_words` (`#[serde(default)]`), `mentions_a_table`; 2 tests. |
| `apps/desktop/src-tauri/resources/tabular-questions/{fr-FR,en-US}.json` | `table_words` (25 and 23 words). |
| `apps/desktop/src-tauri/tests/tabular_pack_parity.rs` (new) | The pack-parity test: same keys at every depth, no empty list, no spelling twice, `table_words` present. 4 tests. |
| `docs/DECISIONS.md` | One section at the end: "Taken while building TABULAR step 02". |

## Measured: names with spaces, before the change

`resolve_in_question` (first hit only), inventory `Ventes 2025 (final).xlsx`, `Mes ventes.xlsx`, `ventes 2025.xlsx`, `Bilan été 2025.xlsx`, `/documents/Suivi commandes clients.xlsx`:

| Typed | Route and result |
| --- | --- |
| `Ventes 2025 (final).xlsx`, quoted or not; `VENTES 2025 (FINAL).XLSX` | `FileNameSuffix` on the last token `final).xlsx`: right, by luck of that token being unique |
| `Mes ventes.xlsx`, quoted or not | `FileNameSuffix` on `ventes.xlsx`: right; **wrong as soon as a `Ventes.xlsx` exists** (exact case-insensitive name wins) |
| `ventes 2025.xlsx` | `Ambiguous` on `2025.xlsx` (two files end with it) |
| `Bilan été 2025.xlsx`, `bilan ete 2025.xlsx` | `Ambiguous` on `2025.xlsx` |
| `Suivi commandes clients.xlsx` | `FileNameSuffix` on the last token: right |
| any of the names without extension, with accents or capitals (`BILAN ÉTÉ 2025`, `ventes 2025 (final)`, `mes ventes`) | `StemToken` through the multi-word phrase route: right (stem of at least 8 characters) |
| a short name (`Q1 24`) without extension | not reached (below 8 characters) |

After: `resolve_all_in_question` resolves every row above, quoted (`"..."`, typographic and guillemets) or not, with or without extension, in any case and accent, including a foreign key (`/documents/...`); a short name resolves when quoted or written with its extension. Known limits, kept as tests that assert "unresolved": `Q1 24` unquoted without extension; a part of a name (`final 2025`); another word order.

## Her real workbooks (copy of her index, throwaway test deleted)

Her `settings.json`: Documents folder `C:\Users\elise\AssistantCabinetAI-test\Docs`, Data folder `C:\Users\elise\AssistantCabinetAI\Data` (not the test Data folder). 18 workbooks in the tables family: 3 of them lie in the Documents folder (`/documents/administratif/commandes/...`) with status `Discovered`, so they have no usable inventory and no card; the other 15 each give a card of 153 to 407 characters. All 15 together are 3 902 characters on the 4 000 budget, flagged truncated because some cards were shrunk by the equal sharing. The cards are printed in the French report. Questions run on her names: `revenue_sub_agency.xlsx` alone names only the xlsx (not the csv of the same stem); two names in one question come back in the order written; `factures-test` (a stem) names `factures-test.csv`; "what is in the spreadsheet" implies nothing with 15 usable workbooks ticked (rule 2 needs at most 3).

## Decisions taken

See the DECISIONS section; in short: `CARD_SHARE_CHARS` 4 000 (not 2 500, arithmetic in the comment); `build_cards` for the equal sharing (a built card cannot be shrunk by `join_cards`); the card takes the key as an input; names are cleaned; the implied rules as built (rule 2 only with nothing named and at most 3 ticked, rule 3 equal to the engine's, rule 4 "some but not all"); `not_selected` holds typed names.

## Measured results

- Pre-check, before the first edit: `file_reference::`, `tabular_answer::`, `tabular::question::` unit tests, 93 passed, no failure.
- Targeted runs during the work: `tabular::card::` 19, `file_reference::` 36 (including the existing 27), `tabular_answer::` + `tabular::question::` all green, `tabular_pack_parity` 4. Two first-run failures were my own expectations and were fixed in the tests, not the code: a repeated name left its second mention uncovered (a real gap, fixed: every occurrence of a whole name is now marked), and `ventes` is genuinely the stem of `Ventes.xlsx`.
- rustfmt: `tabular/card.rs` and `tests/tabular_pack_parity.rs` clean; `file_reference.rs` 4 → 4 `Diff in`, `tabular_answer.rs` 48 → 48, `tabular/question.rs` 21 → 21 (my additions formatted, existing code untouched).
- Final pass, once, after the last code edit: `cargo test -j 2 -- --test-threads=1`: 88 test binaries, 2 067 passed, 0 failed, 59 ignored. `cargo clippy -j 2 --tests`: no warning on a line added by this step (the warnings listed belong to existing code next to it). `pnpm run build` OK; `pnpm run test` 114 files, 2 852 tests passed (the language guard covers the new Rust sources).

## Corrections made to later prompts

- `SESSION-TABULAR-03-advisory-turn.md`: a new block "What step 02 delivered" (real names, `build_cards`/`CardBlock`, `CARD_SHARE_CHARS` 4 000, the rules as built, what the caller passes for `entity_candidates`, the caller drops `implied_single` for doors 1-3); item 1 rewritten around `build_cards`; the window test updated (about 10 000 characters used, about 8 000 left); `join_cards` mention replaced by `CardBlock.text`.
- `SESSION-TABULAR-04-usage-trace.md`: a new block "Names you inherit from step 02" (`not_selected` holds typed names, `ambiguous` holds the typed query and keys, `as_code()`, `card_truncated` per card and what makes it true); `card_truncated` now reads `WorkbookCard.truncated`.
- `SESSION-TABULAR-00-PLAN.md`: section 4.2 gains an "As built by step 02" paragraph.
- Read, nothing to change: `05`, `06`, `08` (08 only says `named_workbooks`, whose keys and order are as assumed).
