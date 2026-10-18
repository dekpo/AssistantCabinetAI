# Speech phase 1: contracts and decisions (lot report)

Date: 18 October 2026. Branch to create: `feat/speech-contracts`, from `feat/speech-integration` (the working tree was clean on
`feat/speech-integration` when the work started; the changes below are uncommitted). Documents only: **no code changed**.

## What was done

The speech work is allowed, specified and bounded before any code. The owner reads `docs/SPEECH.md` and the decision table of
`docs/DECISIONS.md` and answers yes or no.

Read first: SpeechLab `docs/integration/02-needs-and-structure-analysis.md` and `03-integration-plan.md` (draft v1, ON HOLD banner
superseded by D1 to D14), `docs/TTS_LICENSES.md`, then every fact re-checked in the code of `feat/speech-integration` (list at the
end of `docs/SPEECH.md`, section 15). Nothing in SpeechLab was modified.

## Files created

| File | Content |
|---|---|
| `docs/SPEECH.md` | The canonical spec: architecture (two workers, JSON-lines protocol, ports), settings, language table and packs, machine codes, flows (dictation, names, notes, read-aloud), privacy table, models and download, licences, minimum configuration, invariants and named tests, phases and order, spikes, open points |
| `.cursor/rules/speech-programme.mdc` | The programme ritual, modelled on `kb-programme.mdc` |
| `docs/SESSION-SPEECH/STATUS.md` | Local status: phase table, open points (git-ignored) |
| `docs/test-reports/speech-programme/01-contracts-and-decisions.md` | This report |

## Old voice rules amended (one line each)

| File | Amendment |
|---|---|
| `AGENTS.md` | "voice" removed from the frozen list; a bullet says voice is no longer frozen, within the speech decision, with the rule file; `docs/SPEECH.md` added to the "as relevant" reading list |
| `.cursor/rules/v0-sprint.mdc` | The "No longer frozen" paragraph: Whisper-first removed, voice's own decision exists since 17 Oct 2026, Parakeet CC BY accepted by it, free-licence rule kept for every other model |
| `.cursor/rules/architecture.mdc` | Point 5: `/v1/jobs` reserved, `/v1/audio/*` reserved and **unused** |
| `docs/ROADMAP.md` | The "Speech (Whisper first), in SpeechLab" track row replaced by the voice track row; a new subsection "The voice track" (capabilities V-A to V-F, my labelling); the risk row "Drift onto Open WebUI..." amended; a new voice risk row; the paragraph "Moved to planned on 8 October 2026" says voice's decision was taken |
| `docs/ARCHITECTURE.md` | The reserved-routes row split (`/v1/jobs` reserved, `/v1/audio/*` reserved and unused); the four speech ports added to "Workstation capabilities" |
| `docs/DECISIONS.md` | "Reserved contracts": amended note and the two audio routes marked unused; the client UI row "Per-answer action row" (read-aloud) amended; the row "Dictating a question" marked superseded (whisper.cpp sidecar and the 14 October date withdrawn, `webkitSpeechRecognition` rejection kept); KB D7 amended (D2 of the speech section **is** the explicit decision for a CC-BY model; free licences stay for every other model); KB D8 amended (no auto-read); KB "No longer frozen" row points to the new section; **new section** "Speech: workstation-local dictation, read-aloud and voice notes (17 October 2026)" recording D1 to D14, the restated voice rules, the invariants, the 18 October decisions and the open points |
| `docs/PRIVACY-AND-SECURITY.md` | The row "Speech hints, transcript repair, the outbound guard (lot 9)": **only its first sentence** replaced by the D7 three-guard rule (worded as the rule `speech_repair_names` must obey; the outbound-guard sentences untouched); two rows added to "Who stores what" (audio, models); a new section "Speech: audio, workers and the model download" |
| `docs/LANGUAGE-AND-LOCALE.md` | New section on the speech locale mapping, the packs as data, de and it anticipated |
| `docs/CLIENT.md` | The `WorkspaceView` union corrected to the real kinds, `welcome` and `files`; the "voice notes add a kind" sentence replaced; new subsection "Voice in the window" (Notes as a section with a Files / Notes switch, pane, mic button, read-aloud button, settings block) |
| `docs/HARDWARE.md` | New subsection "Client workstation: speech", a pointer to `docs/SPEECH.md` |
| `models/LICENSES.md` | **Not touched** (blocked by `.claudeignore`, not read, columns not guessed): the rows to paste are in `docs/SPEECH.md`, section 10 |

## Things you should look at

1. **The voice licences.** The free-licence policy of `docs/MODELS.md` names Apache 2.0 and MIT; Coqui css10 is BSD-3-Clause, `libritts_r`
   CC BY 4.0, `siwis` rated `review`. I wrote D3 as the explicit decision for those voices and kept the policy for chat models. **Confirmed by the
   owner, 18 October 2026** (open point 1).
2. **The Parakeet attribution text** is my wording of the CC BY 4.0 duties (credit, licence link, indication of changes: int8 and ONNX
   conversion). I did not read the model card (no network used); phase 3b checks the wording when the model is pulled.
3. **The minimum and recommended computers** in "Minimum configuration" are **estimates** (labelled), reasoned from the laptop's figures,
   to be replaced by the new workstation's measurements at phases 3b, 9 and 13.
4. **Choices I made where the prompts were silent** (all easy to change): the protocol has no `cancel` message (cancel is a process kill; 3b may
   add one); the capability labels V-A to V-F in the roadmap; the `ttsRate` clamp and the `ttsEngine` ids are left to phase 2.
5. **Voice licence of siwis** is recorded exactly as decided on 18 October: accepted for the controlled prototype, settled before any wider
   distribution, retraining a later track.
6. `00_LISEZMOI_ordre.md` ("How to use") still says "merged into `feat/document-viewer`"; the rule is `feat/speech-integration` (noted in the
   status file; the file is local and I left it as it is).

## Verification

Documents only, so no test suite was run (no code, no fixture, no catalogue changed; the language guard scans `src-tauri/src` and the
catalogues, neither touched). Checks done: every cited file and fact read in the code; the edited files keep their CRLF line endings
(the two new Markdown files and the new section were converted to match); `git status` shows only the files listed above.

## Human test

Nothing is visible in the application. The nearest check is a reading by the owner: `docs/SPEECH.md` (about 15 minutes; sections 2, 7, 10, 11
and 14 are the ones that need her yes or no), then the decision table in `docs/DECISIONS.md`.
