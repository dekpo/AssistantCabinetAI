# Speech phase 2: foundations, light (lot report)

Date: 18 October 2026. Branch: `feat/speech-foundations`, from `feat/speech-integration` (same commit `d453670` when the work
started, working tree clean; the changes below are uncommitted).

## What was done

The smallest backend base every later speech phase needs: a `speech` module with named constants, the data-driven language table,
the nested `speech` settings, the 22 machine codes with their sentences in both languages, and one command, `speech_status`. No UI,
no engine, no audio, no thread, no file access, no network. The application has no `.setup()` hook and the new state is built empty,
so nothing speech-related runs at launch.

## Files created

| File | Content |
|---|---|
| `src-tauri/src/speech/mod.rs` | Module doc; named constants with units (dictation and note caps in seconds, read-aloud rate bounds, id lengths) |
| `src-tauri/src/speech/languages.rs` | The table: interface locale, engine code, rule pack names, `enabled`/`planned`, capability flags, default voices by gender key. `fr-FR` and `en-US` enabled; `de-DE`, `de-CH`, `it-IT` planned. `resolve(locale, feature)` answers `speech_locale_unsupported` for unknown, planned or incapable rows. `table()` is what `speech_status` hands the interface |
| `src-tauri/src/speech/settings.rs` | `SpeechSettings` (camelCase, `serde(default)`, `sanitised()`) |
| `src-tauri/src/speech/status.rs` | `compute(&Settings, &Probes)`: pure function, per feature `available` plus the machine code of the first reason |
| `src-tauri/src/speech/tests.rs` | 27 tests (see below) |
| `src-tauri/src/speech_commands.rs` | `SpeechState` (its own managed state, holds the probes) and the command `speech_status`, a shell over `status::compute` |
| `src/components/voiceOffBaseline.test.ts` | The D13 baseline, written **before the first edit** and shown green on the untouched code: Composer (idle with a draft, busy, empty draft, needs-folder), MessageList (finished question and answer, answer being written), TitleBar. Exact markup as inline expectations (no snapshot files). It passes unchanged after the edits |
| `src/lib/speechSettings.test.ts` | Both sentences exist for each of the 22 codes with the same `{values}`; the Rust file names exactly these speech codes; the TypeScript defaults |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/lib.rs` | `pub mod speech;`, `mod speech_commands;`, one `.manage(speech_commands::SpeechState::new())`, one handler name |
| `src-tauri/src/error.rs` | 22 variants with their `code()` arms and their `data()` arms (`SpeechLocaleUnsupported { locale }` carries the tag; the others carry nothing) |
| `src-tauri/src/settings.rs` | The field `speech`, its default, sanitising in `load()` and `save()`, the existing test `the_stored_shape_is_the_one_the_interface_expects` completed; `clean_folder_name` now calls a new `pub(crate) clean_folder_name_or(typed, fallback)` (same behaviour, the fallback is a parameter) |
| `src/lib/ipc.ts` | `SpeechSettings` interface, `DEFAULT_SPEECH_SETTINGS`, the field `speech` in `AppSettings` |
| `src/locales/fr-FR.json`, `src/locales/en-US.json` | 22 sentences under `errors` |

`commands.rs` is **not** touched. Helpers widened or duplicated: none (`AppState.settings` is public and was enough). `SpeechState::probes` is
`pub(crate)` because the launch test reads it.

## Decisions taken where the prompt left a choice

- `ttsEngine` is `null` (automatic) or `"piper"` or `"system"`; any other value reads as `null`.
- `ttsRate` is clamped to 0.5 to 2.0 (default 1.0); a value that is not a number reads as 1.0.
- Caps: chat dictation 10 to 600 s (default 120), note dictation 60 to 3600 s (default 1800).
- `languageOverride` is kept if it looks like a tag (letters, digits, hyphens, 16 characters at most); whether it is supported is the language table's answer, not the settings'.
- `speechModelsUrl` must start with `https://` or `http://`, otherwise the default is used. **The TypeScript default is the empty string**, because
  `guards/viewer.test.ts` forbids any URL in a webview source; Rust reads an empty address as its own default and the webview never holds one.
- Voices of `en-US`: `ljspeech` under the gender key `female`, `libritts_r` under `mixed` (a multi-speaker model). `fr-FR`: `siwis` female, `css10` male.
- Locale matching is exact (case-insensitive) on the table's ids: `fr` alone, or `fr-CH`, answers `speech_locale_unsupported`.
- Notes are available when `notesEnabled` is on and a documents folder is chosen (a setting check, no disk access); otherwise `notes_folder_unavailable`, or `speech_unavailable` when their own switch is off.

## Tests

Named tests of `speech/tests.rs`: defaults and documented values, camelCase round trip, file written before speech existed, partial block and unknown keys,
caps and rate clamped, notes folder name falls back to `Notes`, engine/voice/language/URL fallbacks, French and English enabled, German and Italian
declared but unsupported, unknown or empty locale unsupported, incapable row unsupported, unique rows, default voices, the table as the interface gets it,
status with everything off, notes independent of the master switch, blank folder, missing pieces reported in order, per-feature switches under the master
switch, the effective locale, unsupported language blocking both voice features, the status over the bridge, the 22 codes, **launch does no speech work**
(no `.setup()`, state registered empty, a fresh state has found nothing), and **the foundation files start no process, thread, connection or file access**.

Final pass, once, after the last edit:

| Check | Result |
|---|---|
| `cargo test --lib --tests --no-fail-fast` | 1645 passed, 0 failed, 54 ignored (the ignored ones were ignored before) |
| `cargo clippy --lib --tests` | no warning in any file of this phase (the crate's 22 existing warnings are elsewhere, not investigated) |
| `rustfmt --check` | the files created are clean; `settings.rs` has 1 `Diff in` block, as before the edit |
| `pnpm run build` | passes |
| `pnpm run test` | 54 files, 1374 tests passed |

Pre-check before the first edit (targeted, `settings::` and `error::`): 27 passed.

One flaky test seen and reported: `embedding_failures::a_failed_file_is_tried_again_by_the_next_pass` failed once when the Rust suite ran at the same
time as `pnpm run build` and `pnpm run test` (it uses very short deadlines); it passes alone (15 of 15) and in the clean full run. Not investigated further:
it is outside this phase.

## Baseline captured before the first edit

- Existing Tauri commands (64): `load_app_snapshot`, `save_settings`, `reset_settings`, `choose_work_folder`, `ensure_suggested_work_folder`, `choose_data_folder`,
  `ensure_suggested_data_folder`, `reveal_data_folder`, `reveal_data_file`, `read_data_file_for_viewer`, `read_data_file_readable_text`, `index_data_folder`,
  `data_folder_inventory`, `reset_data_index`, `check_server_health`, `cancel_chat`, `index_work_folder`, `ask_with_sources`, the `knowledge_*` commands,
  `has_indexed_documents`, `profile_index_status`, `cancel_analysis`, `vector_share`, `find_files`, `work_folder_inventory`, `reveal_work_folder`,
  `reveal_work_file`, `read_work_file_for_viewer`, `read_work_file_readable_text`, `reset_index`, `fill_preview`, `fill_generate`. One is added: `speech_status`.
- `AppSettings` fields before: `locale`, `theme`, `serverUrl`, `modelAlias`, `embeddingAlias`, `workFolder`, `dataFolder`, `answerIdleTimeoutSeconds`,
  `generatedFolderName`, `showDiagnostics`, `writeTimingLog`, `knowledgeMode`, `knowledgePacks`, `knowledgeMinScopeFiles`, `knowledgeMaxSelectivity`,
  `knowledgeMinReduction`, `knowledgeMaxCandidates`, `knowledgeWidenBelowScore`, `knowledgeHideNamesInSuggestions`, `copySources`, `ownNames`, `layout`. One is added: `speech`.

## Things you should look at

1. The empty TypeScript default for `speechModelsUrl` (see above): phase 4's settings block must show the address Rust holds, not this default.
2. `speech/tests.rs` lists the four foundation files in `FOUNDATION_FILES` for the no-process, no-network scan; a later phase that adds an engine adds its own files and does not edit that list.
3. The branch name `feat/speech-foundations` was already used by the merge #42 (same commit as `feat/speech-integration`, no difference); if you prefer a clean name, create `feat/speech-foundations-2` before the commit.

## Not done, on purpose

No UI, no `SpeechSettings.tsx`, no capture, no engine, no resource files (`resources/speech/` arrives with phase 7), no model, no `Cargo.lock` change.
