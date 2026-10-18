# Speech phase 10: notes, files backend (lot report)

Date: 18 October 2026. Branch: `feat/speech-notes-backend`, from `feat/speech-integration` (phase 4 merged, PR #44; working tree clean when the work started; the changes
below are uncommitted).

## What was done

Part A only: two Tauri commands, `notes_preview` and `notes_save`, thin shells over pure functions of `speech/notes.rs`. They save reviewed notes as new files in
`<documents folder>/<notes folder name>`. No speech, no engine, no audio, no UI. **Part B has no Rust code**, as decided in the re-baseline: the per-file path of
indexing is not separable (`run_cancellable` calls `retain_documents` on the whole folder; the single-flight guard is the `inFlight` ref of `useIndexing`), so Save
will run the existing full Analyse through the App's `indexing.run()`. The interface-side helper that counts the OTHER pending files is built and tested here.

## Files created

| File | Content |
|---|---|
| `src-tauri/src/speech/notes.rs` | `NoteDraft`, `NoteFormat`, `NoteFile`, `Target`, `SaveLog`; `preview(target, drafts, now)` and `save(target, drafts, now, log)` |
| `src-tauri/src/speech/notes_tests.rs` | 23 tests on temporary folders |
| `src/lib/notesAnalysis.ts` (+ test) | `pendingOtherDocuments(files, savedPaths)`, `needsAnalysisConfirmation(...)`: reuses `countPendingDocuments`, minus the notes just saved, case-insensitive on paths |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/commands.rs` | `work_folder_policy` widened to `pub(crate)` (one word) |
| `src-tauri/src/speech/mod.rs` | `pub mod notes;`, `#[cfg(test)] mod notes_tests;`, two doc lines |
| `src-tauri/src/speech_commands.rs` | `notes_destination` (private) and the two commands |
| `src-tauri/src/lib.rs` | the two handler names |
| `src/speech/ipc.ts` | `NoteFormat`, `NoteDraft`, `NoteFile`, `notesPreview`, `notesSave` |

`error.rs` and the catalogues are not touched: the six notes codes already existed. `FOUNDATION_FILES` is not touched. Helpers duplicated: `create_unique` of
`fill_plan` was copied into `notes.rs` (its candidate-name function is shared by the preview and the save, so the names they give agree).

## Behaviour

- **Destination derived in Rust**: the documents folder comes from the settings and is revalidated by `WorkFolderPolicy::validate`; the subfolder is
  `settings.speech.notes_folder_name`, cleaned once more by `clean_folder_name_or(.., "Notes")`; a symlink or junction, a file in its place, or a resolved path that
  leaves the documents folder gives `notes_folder_unavailable`. A preview never creates the folder; a save creates it (`create_dir`, one level).
- **Name**: `YYYY-MM-DD_HH-MM-SS` (local time, passed in as a clock), then `-` and the user's name cleaned by `sanitize_file_name` (dots become `-`, 80 characters at most);
  an empty name gives the timestamp alone; a name with nothing usable once cleaned (punctuation, characters with no ASCII form) gives `notes_name_invalid`.
  Extension `.md` (default) or `.txt`. Collisions get `-2`, `-3` through `create_new`; case-insensitive within one batch.
- **Bytes**: text exactly as reviewed, a leading BOM dropped, `\r\n` and `\r` turned into `\n`, UTF-8, no header.
- **Limits**: 20 notes (`notes_too_many`), 1 MiB each after normalisation (`notes_too_large`); every draft is checked before the first write.
- **Whole or nothing**: if a write fails, the files this very call created are removed and `notes_write_failed` is returned. This is the only removal, and only
  of paths just returned by `create_unique`.
- **Log**: `saved-notes.jsonl` in the app-local data dir, one JSON line per file (`at`, `output`, `bytes`, `chars`, `lines`), no text; best effort.

## Tests

Pre-check before the first edit (targeted): `cargo test --lib speech` 27 passed.
Final pass, once, after the last edit: `cargo test --lib --tests --no-fail-fast -j 2` all green (955 passed, 4 ignored in the library, every other binary ok);
`cargo clippy --all-targets`: no warning in a file of this phase (warnings listed are in other files, pre-existing, not investigated); `pnpm run build` ok;
`pnpm run test` 62 files, 1500 tests passed; `rustfmt --check` clean on the two new Rust files; `speech_commands.rs` had 0 diff blocks before and after.

Covered: never overwritten and numbering (across saves and within one), preview equals save and writes nothing, revalidation (missing, forbidden, file instead of
folder, link instead of folder with nothing landing behind it: the link was really created on this machine, the test did not skip), UTF-8 / no BOM / LF, round trip
through `extraction::extract`, limits (20 / 21 notes, exactly 1 MiB / 1 MiB + 1), invalid names, log without text and a log that cannot be written, camelCase bridge,
source scans (no network / process / thread in `notes.rs`; no `kb_`, SQL or indexing in `notes.rs` and `speech_commands.rs`), no `.setup(` and both handlers
registered.

**Not proven by a test**: the mid-batch write failure itself (it cannot be provoked portably); only the rollback helper is driven directly. The two commands
themselves (they need an `AppHandle`) are covered by reading, as everywhere in the repository.

## For the next phases

- Phase 11 (notes mode, typed) calls `notesPreview` / `notesSave`, then `indexing.run()`; it asks first when `needsAnalysisConfirmation(files, saved.map(f => f.relativePath))`
  is true; a failure of that pass is shown with `notes_analysis_failed`, and the save stays.
- `notesSave` returns the relative paths as the index keys them (the folder name is read back from the disk, so its capitalisation matches).

## Human test

None proposed: this phase has nothing visible (no UI, no button). Its first human use is phase 11's.
