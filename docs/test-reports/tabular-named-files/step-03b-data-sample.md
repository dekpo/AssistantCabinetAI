# TABULAR step 03b - A controlled data sample for the writer (22 October 2026)

Branch `feat/workbook-data-sample`, from `feat/tabular-integration` at the merge of step 03 (`67c2cd7`). The choices are in `docs/DECISIONS.md`, "Taken while building TABULAR step 03b"; this record is what changed, what was measured, what went wrong and what was corrected in the prompts that follow.

## What changed

- **`src/tabular/sample.rs` (new, pure, no `tauri::`).** `build_sample(requests, options) -> SampleBlock { text, uses }`: the rows of every workbook the question names or concerns (whole when they fit, otherwise the first rows and rows spread evenly, the last one always included), the engine's column profile over all the rows, identifiers masked, cells cleaned, everything between `<<DATA-token>>` and `<<END-token>>` (token drawn per request by `fresh_token`). `SAMPLE_INSTRUCTION`, `SAMPLE_SHARE_CHARS` (1 600), `PROFILE_SHARE_CHARS` (1 000), `sample_budget_chars` and `profile_budget_chars` (scaled in both directions: a sample is dropped whole on a window too small for it), `Masker`, `figure_text`.
- **`number_check::arithmetic_expressions`**: the explicit arithmetic a model wrote (`10 + 40 = 50`). `MixedAnswer.model_arithmetic`, a caution line under the answer, read aloud, the answer never edited.
- **`mixed_answer.rs`**: `MixedContext.sample_text`; `build_turn(document_sources, cards, sample, table_block)`; `SAMPLE_INSTRUCTION` appended only with a sample; the formula sentence says `because you cannot run it on the data` when a sample is present; the sample joins the known texts of `verify_numbers` and of the unverified-numbers check.
- **`tabular_answer::question_samples`**: builds the block from the typed workbook cache the answer path already uses. **`commands::mixed_tier`** reads the setting, draws the token, builds the block before the first `.await`, and passes the counts to the trace.
- **`settings.rs`**: `TabularRowsMode` (`off` | `sample`, default `sample`), `Settings.tabular_rows_mode`, re-exported by the crate. **`workbook_use.rs`**: `rows_sent`, `rows_total`.
- **Interface**: the selector in `SettingsDialog.tsx` (« Données envoyées à l'IA depuis vos tableurs »), the Sources line `structure and N rows of M, identifiers masked`, `ChatEntry.workbookCards` as a list of uses, the caution line `chat.modelArithmetic`; keys in both catalogues.
- **`inventory::decimal_mark`** (the decimal mark of the interface language, for typed numbers).

## Measured on the real models

Throwaway harness (deleted), the real turn built by the real code from copies of four of her fictional files (`factures-test.csv`, `fournisseurs.csv`, `reglements.csv`, `revenue_sub_agency.csv`), the tower `http://192.168.1.83:8080`, temperature 0.1, six runs per cell, each model with the window it publishes (8 192, 16 384, 12 288 tokens). 186 answers analysed in all, every flagged one read (draft wording 72, final wording 72 + the re-measures of the questions the last change touched, control 6).

| Question | Model | Refused for lack of data | Arithmetic found | Figures reported as in no source | Quotes the profile |
| --- | --- | --- | --- | --- | --- |
| `Résume factures-test.csv` | `qwen3:4b-instruct` | 0 / 6 (4 / 6 with structure only in step 03; control with `off` in this harness: columns only, no figure, 6 / 6) | 0 | 0 | 6 / 6 |
| | `ministral-3:3b` | 0 / 6 | 0 | 0 | 6 / 6 |
| | `gemma4:e4b` | 0 / 6 | 0 | 0 | 6 / 6 |
| `Explique-moi comment rapprocher ...` | `qwen3:4b-instruct` | 0 / 6 | 0 | 0 | 4 / 6 |
| | `ministral-3:3b` | 0 / 6 (1 / 6 says a formula cannot be tested) | 0 | 0 | 2 / 6 |
| | `gemma4:e4b` | 0 / 6 | 0 | 0 | 2 / 6 |
| `Que peux-tu me dire de ce tableur ?` | `qwen3:4b-instruct` | 0 / 6 | 0 | 0 | 6 / 6 |
| | `ministral-3:3b` | 0 / 6 (4 / 6 list "no data on ..." as limits) | 0 | 0 | 6 / 6 |
| | `gemma4:e4b` | 0 / 6 | 0 | 0 | 6 / 6 |
| `Résume revenue_sub_agency.csv` (35 854 rows, a sample of 12 / 27 / 20) | `qwen3:4b-instruct` | 0 / 6 | 0 | 0 | 6 / 6 |
| | `ministral-3:3b` | 0 / 6 | 0 | **3 / 6** (870 written « 8 700 », sampled amounts rounded) | 4 / 6 |
| | `gemma4:e4b` | 0 / 6 | 0 | 0 | 6 / 6 |

Over all 186 answers the arithmetic check fired twice, both on `ministral-3:3b` answering the overview question, both on **true arithmetic done by the model on rows it saw** (`10+40+70=120` and its two siblings, `450 + 620 = 1 070`): no false positive, including on the join answers, which are full of formulas. Cost of the block: 3 to 120 ms for the small files, 2.1 to 2.4 s for the 35 854-row file (debug build, workbook cache already holding the file).

**The draft wording had one defect.** Its formula sentence (« say you cannot test it because you do not see the data ») made `qwen3:4b-instruct` write « the data is not visible here » in 3 of 12 runs, which the sample contradicts. The replaced phrase (`because you cannot run it on the data`, only when a sample is present) removed it (0 of 12). The other four sentences of the draft were kept as written.

**Her index:** the figures of the human test (profile of the three small tables and of the big one) were recomputed by the real code on a copy of her index (the original never opened for writing): identical to the copies of her files.

## Problems found and fixed during the work

- A ten-digit amount of the big table (`3278225884`) was masked as a telephone number by the English-language scheme of the base pack. A bare run of digits without a leading zero is no longer read as a telephone number (known limit written in DECISIONS, row 6).
- With three workbooks the profile of the second and third was cut before its `montant` column while the first left room unused. The unused room now goes to the profiles that had to be cut (re-measured).
- My own first Python edits converted `\n` escapes into real line breaks inside Rust string literals (the tool collapses doubled backslashes): found by `rustfmt --check`, fixed with the edit tool.

## Not decided here, not investigated

- Whether `sample` stays the default once real documents are in view (owner decision 36). **Her next question already sends rows** (no key in her `settings.json`).
- The sample reaches the mixed tier only. Tables-only advisory answers get the same remedy in step 05 (written in its prompt).
- Numbers written differently from a pack's pattern can pass the masking (a bare ten-digit identifier is read as an amount); a name in a cell is sent. The Sources line shows what was sent.
- Pre-existing and not investigated: none seen in the pre-check (81 unit tests of the touched modules, green).

## Corrections made to the prompts that follow

- `04-usage-trace`: new block "Names you inherit from step 03b" (`rows_sent` / `rows_total`, the fifth parameter of `workbook_use::of_mixed_answer`, `WorkbookCardUse`, the Sources line already carries the counts, the caution line already written, `sample_text`, `build_turn` with four parameters).
- `05-tables-only-advisory`: new block (the sample as the remedy for the tables-only writer, `question_samples`, the known texts).
- `06-explain-and-analyse-calculation`: new block (`sample_text` in the known texts, Analyse carries the sample, Explain does not, `arithmetic_expressions` on the explanation).
- `07-checked-formula-help`: new block (`MixedAnswer` literals, **the formula sentence and its replacement**, the formats a sample shows).
- `10-rows-chosen-by-engine`: new block (names of the module, the shares and their headroom of about 150 characters, the block shape, the setting and where step 10 adds `auto`, the helpers to make `pub(crate)` and reuse, the checks already built).
- `00-PLAN`: section 1.2 (the optional fourth block), section 1.4 (`MixedContext` and `sample_text`), a paragraph "As built by step 03b" before 4.5.
- Read, nothing to change: none read beyond these five and the plan.

## Tests

- Pre-check before the first edit: the unit tests of the touched modules (`number_check`, `workbook_use`, `settings`, `mixed_answer`, `tabular::card`), 81 passed.
- New: `tabular::sample` (17 unit tests: small table whole, large table head plus spread and deterministic, roomier and tiny windows, wide table cut, cells as the file shows them, a sheet that is not a table, masking of a French health insurance number, an AVS number, telephone numbers, e-mail addresses and IBANs with names left alone, a ten-digit amount left alone, a column of identifiers never listed, a hostile cell, the profile's figures, shared room, neutral wording), `number_check` (3), `workbook_use` (1 new, 1 updated), `settings` (1), `mixed_answer` (1 new, the window test updated); `tests/tabular_sample.rs` (6 integration tests on the counting gateway: every figure of the profile is the engine's own answer on `factures.csv` and `agencies.csv`, the turn carries the markers and the sample, the turn with the setting `off`, a hostile cell on the real request, the arithmetic, a quoted and an invented figure); vitest (the Sources line with the row counts, the caution line, the selector).
- The step 01 and 03 baselines (`tests/mixed_routing_baseline.rs`, helper `mixed_with_rows_mode`, `Off` in the frozen tests) pass unchanged.
- Final pass, once, after the last edit: `cargo test -j 2 -- --test-threads=1` **2 111 passed, 0 failed, 59 ignored** (1 254 unit tests); `cargo clippy --lib --tests` no finding in the code written here (23 warnings in the library, in code this step did not write); `pnpm run build` green; `pnpm run test` **2 858 passed** (114 files; the first run failed on two non-ASCII quotation marks in a Rust comment, found by the source guard and fixed, then green; that edit was a comment, made after the last `cargo` run). `rustfmt --check`: `sample.rs` and `tests/tabular_sample.rs` are clean; the files modified have no more `Diff in` blocks than before.

## Owner human test (22 October 2026)

Played by the owner; results, findings and what was not investigated: `step-03b-human-test-results.md` and DECISIONS, "Owner's test of TABULAR step 03b". Verdict: the sample, the profile, the masking and the Sources line work; one defect to reproduce before the merge decision (a generic question about three ticked tables reached the writer without any card, finding 1).
