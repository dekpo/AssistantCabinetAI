# Speech phase 11: Notes mode (typed) in the workspace (lot report)

Date: 18 October 2026. Branch: `feat/speech-notes-mode`, from `feat/speech-integration` (phase 3 merged, PR #46; working tree clean when the work started; the changes
below are uncommitted).

## What was done

The Notes section of the centre zone, fully usable with the keyboard and without any voice piece: a `Files | Notes` switch on top of the centre, a Notes button in the title bar,
a writing page (several drafts, Raw / Markdown preview), a save block, a two-step save (plan of the exact files, then the write), the analysis of the saved notes through the
App's own Analyse pass, and a question before any unsaved note can be lost. **No Rust code changed** (one permission line in `capabilities/default.json`). Dictation is not
wired: the pill is a finished component, drawn only when the bundle hands it a `dictation` value, and `App` hands `null`.

## Files created

| File | Content |
|---|---|
| `src/lib/notesDrafts.ts` (+ test) | The drafts as plain data (`NotesState`), memory only; add / select / edit / remove / discard all / `markWritten`; `guardExit(state, exit, context)` answering `allow` or `ask` for every exit |
| `src/lib/centreSection.ts` (+ test) | `CentreSection`, `effectiveSection`, `shownCentre(...)`: the one pure function that gives `AppLayout` its `workspace` |
| `src/lib/notesFlow.ts` (+ test) | `saveAndAnalyse(deps, drafts)`: save, mark clean, count the other pending documents, ask if needed, run the pass once |
| `src/lib/notesSession.ts` (+ test) | `createNotesSession(deps)`: the notes and the save state held outside React (subscribable store); phases `idle / previewing / plan / saving / asking / analysing` |
| `src/lib/waveform.ts` (+ test) | `barHeights(level)` and `formatClock(ms)` for the pill |
| `src/lib/windowClose.ts` | `guardWindowClose(hold)` over `onCloseRequested`, `closeWindow()` over `destroy()`; nothing registered outside the desktop shell |
| `src/components/notes/NotesPanel.tsx` | The page: draft switcher, Raw / Preview, text, pill slot, save block, plan, status, error, report. Exports `NotesBundle`, `NotesActions` |
| `src/components/notes/CentreSwitch.tsx` | The `Files \| Notes` tab list (red dot slot for a recording) |
| `src/components/notes/DictationPill.tsx` | The dark pill: microphone at rest, 7 bars driven by `level`, elapsed time, hint |
| `src/components/notes/NotesDialogs.tsx` | `UnsavedNoteDialog` (Cancel / Discard / Review and save) and `AnalysisChoiceDialog` |
| `src/components/MicrophoneGlyph.tsx`, `NotesGlyph.tsx` | Two decorative glyphs |
| `src/components/notes/notes.render.test.ts` | 26 render tests (see below) |
| `src/speech/notesSources.test.ts` | 27 source scans (see below) |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/capabilities/default.json` | `core:window:allow-destroy` added; the description says why |
| `src/App.tsx` | The notes session, the section state, `shownCentre`, the exit guard (`guardLeaving`), the window close guard, the Notes button props, the two dialogs, the guarded folder change / Settings switch / Reset, `refreshToken` |
| `src/components/AppLayout.tsx` | One optional prop `notes`, passed to `WorkspacePanel` |
| `src/components/WorkspacePanel.tsx` | Optional `notes` prop; the old body moved unchanged into `FilesSection`; with `notes` absent the panel returns exactly that |
| `src/components/TitleBar.tsx` | Optional `notes` prop: one extra button between the workspace toggle and Settings |
| `src/components/ConfirmDialog.tsx` | Optional additive `secondary` prop (a third answer between Cancel and the confirming one) |
| `src/components/WorkFolderCard.tsx` | Optional `refreshToken` prop: one extra effect that re-reads the inventory |
| `src/locales/en-US.json`, `fr-FR.json` | 2 new blocks, `centre` (4 keys) and `notes` (about 45 keys) |
| `src/styles.css` | Classes `centre*`, `notes*`, `pill*`, appended |

Rust helpers widened or duplicated: none (no Rust code touched).

## How the centre stays open (`AppLayout`'s `workspace`)

`AppLayout` opens the zone exactly while its `workspace` prop is not `null`. `App` now passes `shownCentre({ workspace, hidden, notesEnabled, section, notesHold })`
(`lib/centreSection.ts`): `null` when the user hid the zone; the file tabs' own view when there is one; otherwise, only if `notesEnabled`, the welcome view when the Notes
section is on screen **or** a draft has text (`notesHold`). With `notesEnabled` off it returns `workspace` or `null` as before. The title-bar workspace button keeps its old
behaviour (hide / show / open the welcome view). Choosing Files with no tab open puts the welcome view in `workspace`, so the zone does not close under the user.

## Decisions taken where the prompt left a choice

- **Exits that ask** (`guardExit`): choosing another folder (only when a folder was already set: the first choice loses nothing), the window close request, turning the Notes switch off in Settings, "Reset" in Settings, and the explicit "Abandon this note" when that note has text. Switching section and opening a file never ask. Hiding the zone asks **only while a recording runs**; `App` has no recording yet, so it never asks today (the function and its test are ready; phase 12 passes the context).
- **The voice master switch does not guard anything**: a typed note does not depend on it, and turning it off loses no text (the re-baseline listed it; it only matters once a recording exists, phase 12).
- **The dialog's "Save" is "Review and save"**: it gives up the exit and opens the plan of the exact files, so a file is only ever written by the click on the plan (D9: voice or a dialog never applies a file action by itself). It also puts Settings out of the way when the exit came from there.
- **"Names editable" in the plan**: the plan lists the exact files; names are edited in each note's name field (Cancel on the plan goes back to it), not inside the plan rows.
- **Save** saves the note on screen; **Save all notes (N)** appears when several have text. Saved notes leave the list; a note edited after the plan stays (edits are refused under a plan anyway).
- **Drafts live in a store outside React** (`createNotesSession`), not in `useState`: a remount of the panel, a hidden centre or a section switch cannot lose them, and the whole flow is tested in node.

## Tests

Pre-check before the first edit (targeted): `voiceOffBaseline`, `render`, `i18n`, `notesAnalysis` tests, 62 passed.
Final pass, once, after the last edit: `pnpm run build` ok; `pnpm run test` **70 files, 1695 tests passed** (rerun once after the two fixes below) (the guard `sources.test.ts` caught a `=> Promise<void>` type read as a text node
in `NotesPanel.tsx` once; fixed, rerun green); `cargo check -j 2` ok (it runs the build script that validates the capability file). No Rust source changed, so `cargo test`, `clippy`
and `rustfmt` were not rerun (nothing to prove).

Covered: drafts (survive a subscription coming and going, the list is never empty, selection moves to a neighbour, a draft edited after the plan stays); every exit through
`guardExit` (8 exits, with and without an unsaved note, and the recording case); `shownCentre` (switch off = as before, Notes alone, a draft keeps it open, hidden wins);
the save flow and the session (no write while the plan is built or shown, preview equals what will be written, cancel writes nothing, one write and one analysis, confirmation only when
other documents are pending, declined = saved and not analysed, failed pass = still clean, failed write = notes kept and no pass, no edit under a plan); the pill
arithmetic; the render of each state; the baseline of phase 2 (`voiceOffBaseline`) unchanged and green; `ConfirmDialog` byte for byte without the third answer, and the third answer
between the other two; the title bar equal to the old one once the Notes button is removed; no storage, no network, no `@tauri-apps/api/core`, no speech call in the notes files;
the new sentences exist in both catalogues; the capability file grants `core:window:allow-destroy` and no other window power.

**Not proven by a test, to look at in the real window:** the window's close request (`onCloseRequested` then `destroy`), the drag of the separator, the layout at the 320 px minimum,
the hidden Files section keeping its viewer's scroll and zoom after a section switch (the viewer is mounted with `display: none`), and the real Analyse after a Save. I did not walk the
protocol below in a running application; the state machine behind it is what the tests walk.

## Things you should look at

1. Quitting from the macOS menu or Cmd+Q does not raise the close request (phase 14).
2. Dragging the centre closed while recording cannot be intercepted (the layout has already collapsed when the callback runs): phase 12 has to decide what a recording does then.
3. `notesSources.test.ts` asserts that `App` hands `dictation: null` and that no notes file calls the speech engine: phase 12 updates those two assertions when it wires the pill.

## Human test (short script, played by the owner on 18 October 2026)

Steps 1 to 8 passed (Documents folder had nothing to analyse, so no confirmation dialog was expected; the plan wrote nothing before the click; analysis ran). Two findings, both fixed in the same lot:
- The unsaved-note dialog showed one run-on sentence; it now shows "N notes are not saved." and "If you go on, they are lost." on two lines (a line break in the catalogue string, and `white-space: pre-line` on `.confirm__body`; no existing sentence holds a line break, checked).
- After a save the fresh note was called "Note 3"; the report said "Note 1". Numbering now starts again at 1 when the list had to be refilled (ids stay unique).

## Prompt alignment

Checked `05b_speech-3b_stt-worker.md` (Rust worker only): aligned, nothing changed. I also added an "Alignment with phase 11" section to `10_speech-12_notes-dictation.md` (it consumes the pill, the bundle and the exit guard); no owner decision needed.

## Human test (about 25 minutes)

The script is in the chat message that hands this lot over; it starts with a backup and ends with one restore.
