# Speech phase 7: read-aloud text preparation (lot report)

Date: 18 October 2026. Branch: `feat/speech-readaloud-text`, from `feat/speech-integration` (phase 3b merged, PR #48; working tree clean when the work started; the changes below are uncommitted).

## What was done

The backend that turns an answer into speakable segments, in Rust, by code, never by the model: a line-based Markdown reader, an inline cleaner, an abbreviation-aware sentence splitter with a length cap, and a scanner that says numbers, dates, times, units, currencies, ordinals, telephone numbers, titles, identifiers and acronyms as words. One command, `speech_prepare_reading(parts, locale)`, a thin shell over `speakable::prepare_for_locale`. No UI, no engine, no audio, no network.

The words of a language are data: `resources/speech/rules/fr-FR.json` and `en-US.json`, embedded with `include_str!` and parsed once (registry in `speakable_rules.rs`, reached through the language table). The logic names no language; a scan test proves it.

## Files created

| File | Content |
|---|---|
| `src-tauri/src/speech/speakable_rules.rs` | rule pack types (unknown keys ignored), registry, `rules_for(locale)` through `languages::resolve`, `parsed()` |
| `src-tauri/src/speech/speakable_numbers.rs` | cardinals, ordinals, years, feminine "one", digits one by one, all from the pack |
| `src-tauri/src/speech/speakable_scan.rs` | the scanner (port of SpeechLab `normalise.rs` as a specification, data-driven); titles from `PackSet::title_spoken` |
| `src-tauri/src/speech/speakable_sentences.rs` | the splitter (specification: SpeechLab `sentences.rs`), `MAX_SENTENCE_CHARS = 240` |
| `src-tauri/src/speech/speakable_markdown.rs` | blocks (text / note), `clean_inline` |
| `src-tauri/src/speech/speakable.rs` | `Part`, `Segment`, `Pause`, `prepare`, `prepare_for_locale`, `speak_sentence`, `reading_allowed`, identifier reading |
| `src-tauri/src/speech/speakable_tests.rs` | 2 tests that need `Settings` (the switches; default packs) |
| `src-tauri/resources/speech/rules/fr-FR.json`, `en-US.json` | the rule packs |
| `src-tauri/tests/speech_speakable.rs` | 53 tests (SpeechLab unit cases retyped, pipeline, scans) |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/speech/mod.rs` | 6 `pub mod speakable*`, `#[cfg(test)] mod speakable_tests;`, a doc paragraph |
| `src-tauri/src/speech_commands.rs` | one `use` line and the command `speech_prepare_reading` (settings read only for `reading_allowed`) |
| `src-tauri/src/lib.rs` | the handler name next to `speech_model_cancel`; still no `.setup(` |
| `docs/SPEECH.md` | an "As built (phase 7)" paragraph in 7.4 and the module layout line |

`error.rs`, the catalogues, `commands.rs`, `Cargo.toml`, `ipc.ts` are **not** touched. Nothing widened to `pub(crate)`; nothing duplicated. No new `AppError` (notes are data of a result).

## Contract

`speech_prepare_reading(parts: [{kind: "markdown"|"plain", text}], locale)` returns an ordered list: `{kind:"speech", text, pauseAfter:"clause"|"sentence"|"paragraph"}` or `{kind:"note", code:"table_omitted"|"code_omitted", count}`. It refuses with `speech_unavailable` (voice or read-aloud switch off) and `speech_locale_unsupported` (de, it, unknown, empty: no pack).

## Decisions of this phase (the owner may overrule any of them)

- **Own Markdown stripper**, no `pulldown-cmark` (open point 6, recommended option).
- **Table limits**: read row by row up to 8 body rows and 5 columns ("Column: value, Column: value"); beyond, a note with the row count. Fenced code: a note with the line count.
- **File names** are read as words (separators become spaces) and the **extension is not said**; paths are reduced to the file name. Bracketed citation numbers (`[1]`) are dropped.
- **Notes are positional** items (they stand where the table was); whether they are spoken is phase 8's call.
- **Identifier reading** by scheme id then kind, from the pack (`identifiers.readings`): the groups as written, else cut by the lengths of the pack; letters by the pack's letter names; e-mail marks said in words. Only schemes with an entry are read this way; the others stay with the number rules.
- **Acronyms spelled**: a short list per language (IRM, ECG, TVA, AVS... / MRI, ECG, VAT...), extendable in the pack.
- **KB `spoken_form` step left out** (open point 8); the command takes no `scope`. A scan test proves the preparation opens no index and resolves no selection.
- **German**: adding it is a table row, a data file and a registry line; tens-after-units grammar also needs a small addition to `speakable_numbers.rs` (stated in `docs/SPEECH.md`, not "no change").
- Number, date, time, unit and ordinal rewriting is always on. Verified on the sentence types the owner listened to (date, time, dose, percentage, amount, telephone, AVS).

## Tests

Pre-check before the first edit (targeted): `cargo test -j 2 --lib speech::` 126 passed, 2 ignored, 0 failed.
Final pass, once, after the last edit: `cargo test -j 2 --lib --tests --no-fail-fast` all green (1800 passed, 56 ignored, 80 result lines, 0 failed); `cargo clippy --all-targets`: no warning in a file of this phase (the other warnings are pre-existing, not investigated); `pnpm run build` ok; `pnpm run test` 71 files, 1732 tests passed (the Rust language guard included); `rustfmt --check --edition 2021` 0 diff blocks on the 6 new source files, the new test files and `speech_commands.rs` (0 before as well).

Covered: both rule packs parse and every enabled language has its pack; unknown keys ignored; locales without a pack refuse; French and English cardinals, ordinals, years, decimals, grouping, units and plural rules, percent, currency, times, dates, fractions, feminine "one", titles, abbreviations, acronyms, telephone numbers; identifiers of the active packs (AVS, IBAN, invoice, e-mail); the splitter cases of SpeechLab plus the length cap; Markdown shapes of real answers (headings, bold, links, lists, quotes, tables small and large, code, URLs, paths, emoji); pauses; parts; idempotence; serialisation; switches; scans (ASCII, not model-facing, spoken forms never leave the selection, no network client, no language literal in the logic, command registered, no setup hook).

**Not proven by a test**: the command itself (it needs a `State`); the sound of any sentence (no engine yet). A probe of four realistic French answers read correctly (dates, amounts, IBAN, AVS, telephone, table rows); the probe was deleted.

## For the next phases

- Phase 8: the TypeScript bridge, `readingParts.ts` sending the shown text, the catalogue sentences of the two note codes, the pause durations, the empty-result case.
- Phase 12: add `dictation_commands` to the same two rule files.

## Human test

None proposed: nothing is visible.

## Prompt alignment

Checked `07_speech-8_readaloud-button.md`: appended "Alignment with phase 7" (bridge and types to add, what to send, refusals, items and pauses, notes and their sentences, empty result, left-out `spoken_form`, scans). `STATUS.md` updated (rows 05b and 06, a note for the next phases). No owner decision needed.
