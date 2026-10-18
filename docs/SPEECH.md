# Speech: workstation-local dictation, read-aloud and voice notes

Canonical specification of the speech programme (written 18 October 2026, phase 1, documents only). The decisions
D1 to D14 are recorded in `docs/DECISIONS.md`, "Speech: workstation-local dictation, read-aloud and voice notes
(17 October 2026)"; this file says how they are built, bounded and tested. Where the two differ, `docs/DECISIONS.md`
wins and this file is corrected.

Reference, read only: the owner's repository `dekpo/SpeechLab` (engine evaluation, TTS licence review, the Rust and
TypeScript code the phases port). Its integration plan v1 is a superseded draft (it still carries an ON HOLD banner,
D-050): the decisions of this repository override it.

Status words: **measured** (a figure SpeechLab observed, on the machine named), **estimate** (a reasoned guess to be
replaced by a measurement), **verified** (read in the code on 18 October 2026), **not verified** (needs a test).

## 1. What it is, and what it is not

Three features, one module, all on the workstation:

1. **Dictation in the chat.** A microphone button in the Composer. Press, speak, press, wait, edit, send. The
   transcript only fills the text box.
2. **Read-aloud.** A button under each finished answer. It speaks the text already on screen, with a local voice.
3. **Voice notes.** A **Notes** section of the centre zone: long dictation into an editable Markdown text, saved as a
   file in the Documents folder, and analysed like any document.

Not in the programme: spoken conversation (no auto-read, no hands-free loop, no wake word), streaming or partial
dictation, a server-side or cloud engine of any kind, voice cloning, accent evaluation, word-level read-along,
speech in `/v1/audio/*`. A voice never confirms or applies a file action.

## 2. The decisions at a glance

Full wording: `docs/DECISIONS.md`. Short form, so that this file can be read alone.

| # | Decision |
|---|---|
| D1 | Base branch `feat/speech-integration` (from `feat/document-viewer`, which stays untouched; safety branch `backup/before-speech`). One branch per phase, `feat/speech-<topic>`, merged back by pull request |
| D2 | Speech to text: Parakeet TDT 0.6B v3 int8 (CC BY 4.0, attribution required) through sherpa-onnx, in a **separate worker executable**. Nothing speech-related is linked into the main executable. whisper.cpp (MIT) may be added later behind the same trait, and is the recorded fallback |
| D3 | Text to speech: Piper through a separate TTS worker (preferred), operating-system voices as fallback **only if proven local**. French: `siwis` (female) and Coqui `css10` (male, to be confirmed by ear); English: `ljspeech` or `libritts_r`. `mls` rejected, `gilles` excluded |
| D4 | No audio, transcript, answer text or name leaves the machine through the speech module. "Local" is proven by an offline test |
| D5 | Models are downloaded on an explicit user action from `https://assistantcabinet.ai/models/speech/`, SHA-256 pinned in the app; the only network code of the module, in one named file |
| D6 | fr-FR and en-US now, German and Italian anticipated: a data-driven language table, locale packs, no `if fr / else en` |
| D7 | Name suggestions after a dictation: selection only, hidden-names setting honoured, nothing new revealed, never a silent replacement, never a write to the KB, generic over entity types |
| D8 | Read-aloud: explicit button, local only; reads the answer, the engine-computed block and the caution notes; Play and Stop only |
| D9 | Notes: title-bar button, own switch, Files / Notes switch in the centre zone, Typeless-like pane, saved in `<documents folder>/Notes`, **Save also analyses** |
| D10 | No recovery copy of unsaved notes on disk |
| D11 | macOS: maximise compatibility now; a Mac is available in November |
| D12 | Test machine: the owner's laptop; the interface never freezes (all engine work out of process) |
| D13 | Non-regression: voice off, every existing component renders exactly as before |
| D14 | A saved note is an ordinary document: once saved and analysed it follows the same path as every document |

## 3. Architecture

### 3.1 Processes

```text
webview (React)                 main executable (Rust)                       workers (separate executables)
  AudioCapture  ---WAV bytes--->  speech_commands.rs (thin shells)
  (getUserMedia,                    speech::service::SpeechService
   AudioWorklet)                      busy flag, cancel token, clock,
                                      private temp-audio folder
  voice session (one mic)             speech::stt (trait) --------------->  speech-stt-worker   (Parakeet, sherpa-onnx
  Notes / Composer / read-aloud       speech::tts (trait) --------------->                        "no-tts" libraries, no espeak-ng)
                                      speech::worker_protocol             speech-tts-worker   (Piper and Coqui voices,
  ReadAloudEngine <---WAV bytes---    speech::model_download (D5)                                 full libraries, espeak-ng GPL-3.0)
   Web Audio playback                 the ONLY file with network code
```

Rules that follow from D2, D3, D4 and D12:

- **The main executable links no engine.** Reason: the full sherpa-onnx libraries contain the GPL-3.0 phonemizer
  espeak-ng, Parakeet holds 1.6 to 1.9 GB (measured), a worker can be killed (real cancellation), and a crash does not
  take the application down.
- **Two workers, two builds.** The STT worker is built against the sherpa-onnx "no-tts" libraries and must contain no
  espeak-ng (a post-build scan for the string `espeak` must find none); the TTS worker is built against the full
  libraries and ships with its own notice file and `espeak-ng-data`. They cannot come out of one `cargo build`: two
  target directories, or two crates. The protocol types, the JSON-lines client, the idle timer and the cancel logic live
  in the **main** crate (`speech/worker_protocol.rs`, `speech/worker_stt.rs`, `speech/worker_tts.rs`) and are tested
  against a fake worker, because CI builds only `apps/desktop/src-tauri`.
- **Cancel is a kill.** A decode cannot be interrupted by a message; the worker is killed and respawned on next use.
  Workers die with the app (no orphan after exit, panic or kill of the parent).
- **Nothing heavy at launch.** Workers start on first use of a feature, never at launch; models load on first request
  and unload after an idle delay (named constant, about 120 s). The temp-audio sweep runs lazily at the first speech
  command (`lib.rs` has no `.setup()`; none is added).
- **Capture and playback stay in the webview**, behind interfaces (`AudioCapture`, `ReadAloudEngine`), so that a native
  implementation can replace `getUserMedia` on macOS without touching a caller (D11).
- **Whole-clip, not streaming.** Dictation is press, speak, press, wait. Notes cut a long recording into segments for
  latency (section 7.3), but each segment is one whole-clip request.
- **One microphone at a time, one speech job at a time.** A small app-level "voice session"
  (`src/speech/voiceSession.ts`, `claim(owner)` and `release(owner)`) is shared by the composer mic, the notes pill and
  read-aloud: starting a recording stops a playback, a playback cannot start while recording, a second recorder is
  disabled with a catalogue sentence. The backend answers `speech_busy` to a second job.

### 3.2 Ports

Business code imports no engine. Both ports follow `OcrProvider`: an id, one call per unit of work, machine-coded
errors, degrade to "unavailable".

```text
SpeechToTextProvider
  availability(language) -> Availability            ready | engine_missing | model_missing | model_corrupt | locale_unsupported
  transcribe(SttRequest, cancel) -> SttResult
      SttRequest { wav_path, language_code (from the language table), max_seconds }
      SttResult  { text, audio_ms, processing_ms, engine_id, case_reliable }
TextToSpeechProvider
  voices(language) -> Vec<Voice { id, gender_key, licence_rating, local }>
  synthesize(text, voice_id, rate, cancel) -> WAV bytes        (temp file deleted at once)
Providers: worker_stt (Parakeet), fake (tests), whisper_cpp (later, same trait); worker_tts (Piper, Coqui);
           system voices live in the webview (ReadAloudEngine), local-only guard
```

`case_reliable` is a property of the engine, carried in its descriptor beside `engine_id`, never a literal in a command:
Parakeet writes cased, punctuated text, and `repair::propose` takes that flag.

### 3.3 Worker protocol (JSON lines on stdin and stdout)

One JSON object per line, in both directions; stderr carries nothing but a numeric diagnostic (no text).

```text
-> {"id":1,"op":"load","model":"parakeet-tdt-0.6b-v3-int8","threads":4}
<- {"id":1,"ok":true,"load_ms":2012}
-> {"id":2,"op":"transcribe","wav":"<private temp dir>/<random>.wav","language":"fr"}
<- {"id":2,"ok":true,"text":"...","audio_ms":10000,"processing_ms":980}
-> {"id":3,"op":"synthesize","text":"...","voice":"fr_FR-siwis-medium","speed":1.0,"out":"<private temp dir>/<random>.wav"}
<- {"id":3,"ok":true,"audio_ms":4200,"processing_ms":210}
-> {"id":4,"op":"unload"}      -> {"id":5,"op":"shutdown"}
<- {"id":N,"ok":false,"code":"speech_engine_failed"}
```

Operations: `load`, `transcribe` (STT worker), `synthesize` (TTS worker), `unload`, `shutdown`. There is **no
cancel message**: cancel is the kill of section 3.1 (phase 3b found no use for one and added none). A worker contains no
network code. Text reaches a worker through stdin, never a command line (a command line is visible to other processes of
the same user). The worker returns codes, never prose.

**As built by phase 3b (the STT worker; this wins over the sketch above).** The whole protocol is one file,
`speech/worker_protocol.rs`, compiled by the application and included by path into the worker crate, so the two sides cannot
drift apart. Requests are tagged by `op`: `load` (`id`, `threads`, `files` = the four full paths `encoder`, `decoder`, `joiner`,
`tokens`, named by the application's manifest, never by the worker), `transcribe` (`id`, `wav_path`, `language`), `shutdown`
(`id`, answered by nothing: the process ends). `unload` is not a message: after the idle delay the application sends `shutdown`,
because ending the process is the only way to give back all the memory of the runtime. Answers are tagged by `status`:
`loaded` (`load_ms`), `transcribed` (`text`, `audio_ms`, `processing_ms`) or `failed` (`kind`: `model_missing`,
`model_corrupt`, `audio_invalid`, `engine_failed`, `bad_request`). A line on the worker's standard output that is not an answer
is skipped, never logged: sherpa-onnx itself writes lines there (for example when it resamples a clip). The worker's standard
error is not read and goes nowhere. Closing the worker's standard input ends it, which is how it dies with the application.

### 3.4 Module layout

```text
apps/desktop/src-tauri/
  src/speech/            NEW, ASCII only, no French marker word, no #[cfg] (platform code goes in a paired module)
    mod.rs  languages.rs  settings.rs  status.rs  service.rs  stt.rs  tts.rs  fake.rs  temp_audio.rs  wav.rs
    output_guard.rs  worker_protocol.rs  worker_stt.rs  worker_tts.rs  models.rs  model_download.rs  speakable.rs  notes.rs
  src/speech_commands.rs NEW, its own managed SpeechState, thin shells over the functions above
  resources/speech/      NEW data: models-manifest.json, rules/<locale>.json (numbers, units, months, abbreviations,
                         dictation_commands)
  binaries/              worker executables staged under their target-triple names (git-ignored)
apps/speech-worker/      NEW crate(s): speech-stt-worker (feature stt), speech-tts-worker (feature tts)
apps/desktop/src/
  speech/                ipc.ts, availability.ts, voiceSession.ts, useDictation.ts, insertion.ts, useLongDictation.ts, output/
  audio/                 recorder.ts, wav.ts, segmenter.ts          public/speech/capture-worklet.js (static, no blob:)
  components/            SpeechSettings, MicButton, DictationStatus, ReadAloudButton, CentreSwitch, NotesPanel, RepairChips
```

Edits to existing files are kept to a registration line each (`lib.rs`, `error.rs`, `settings.rs`, `ipc.ts`, the two
catalogues, `SettingsDialog`, `Composer`, `MessageList`, `TitleBar`, `AppLayout`, `WorkspacePanel`, `App`); every phase
lists its own in its report. Tests drive pure functions and services, never a Tauri command: nothing in the repository
builds a `State` or an `AppHandle` in a test. A command is a thin shell over a function that takes plain arguments.

## 4. Settings

One nested field `speech` of `settings.json` (`SpeechSettings`, in `speech/settings.rs`, camelCase, `serde(default)`,
`sanitised()` on load and on save, mirrored in `ipc.ts`). The webview saves it through `saveSpeech(fn)` in
`useAppSettings` (the top-level `update(patch)` would overwrite sibling fields with a stale copy).

| Field | Default | Meaning |
|---|---|---|
| `enabled` | false | Master switch of dictation and read-aloud (the voice master switch of D13) |
| `dictationEnabled` | true | Counts only once `enabled` is on |
| `readAloudEnabled` | true | Counts only once `enabled` is on |
| `notesEnabled` | **true** | Own switch for the Notes button and section; **independent of `enabled`** (typed notes need no voice) |
| `languageOverride` | null | null = follow the interface locale (`settings.locale`, then `DEFAULT_LOCALE`) |
| `maxDictationSeconds` | 120 | Cap of one chat dictation |
| `maxNoteSeconds` | 1800 | Cap of one note dictation (30 min, D9) |
| `ttsEngine` | null | null = automatic: the Piper worker if installed, else system voices only if proven local, else `tts_no_local_voice` |
| `ttsVoice` | null | A manifest id; null = the first voice of the resolved locale |
| `ttsRate` | 1.0 | Passed as the synthesiser `speed`, never as a playback rate (it would change the pitch); the clamp is fixed in phase 2 |
| `notesFolderName` | "Notes" | Sanitised on the Rust side; an unusable name falls back to "Notes" |
| `notesVoiceCommands` | true | Spoken punctuation, title, subtitle, new line, new paragraph, new note |
| `speechModelsUrl` | `https://assistantcabinet.ai/models/speech/` | Download root (D5); a setting, never a constant |

Values the table does not give (the `ttsRate` clamp, the exact `ttsEngine` ids) are fixed by phase 2 and recorded in its
report. The language is never sent by the webview: Rust resolves it from the settings and the language table.

## 5. Languages and packs (D6)

One data table, `speech/languages.rs`: **interface locale -> engine language code -> rule pack names -> capability
flags -> default voice ids by gender key**. fr-FR and en-US are enabled; de-CH / de-DE and it-IT are declared
`planned`, not enabled. One function resolves a locale and answers `speech_locale_unsupported` for anything else
(unknown, planned, or enabled without the capability the feature needs). No `fr` or `en` literal exists outside this
table, and no `if fr / else en` anywhere. `speech_status` returns the table (id, enabled or planned) so that the
language override list comes from Rust.

| Locale | Engine code | Dictation | Read-aloud | Default voices (gender key -> manifest id) |
|---|---|---|---|---|
| fr-FR | fr | enabled | enabled | female -> `siwis`; male -> `css10` |
| en-US | en | enabled | enabled | `ljspeech`, `libritts_r` |
| de, it | de, it | planned | planned | to choose in phase 15 (licence review first) |

**Rule packs are data**, `resources/speech/rules/<locale>.json`, embedded the way knowledge packs are (`include_str!`
plus a registry): number words, units, months, abbreviations, acronyms to spell, sentence-end exceptions, and (phase 12)
`dictation_commands`. They hold words, never user-visible sentences. The loader ignores keys it does not know (no
`deny_unknown_fields`). Adding German or Italian is a row of the language table plus data files: **no logic change**
(still a source edit, since the packs are embedded). `title_spoken` (dr -> docteur) is read from the knowledge **base**
pack, not duplicated. Every user-visible sentence is a catalogue key in both `fr-FR.json` and `en-US.json`; a model-facing
string does not exist in this module (it adds none).

## 6. Machine codes

Rust returns codes, never prose. Each code is one `AppError` variant with a `code()` arm and a sentence in both
catalogues (`errors.<code>`), enforced by `apps/desktop/src/guards/sources.test.ts`. Webview-side conditions
(microphone denied, no microphone, unsupported, recording failed) are catalogue keys of the webview, not Rust codes.

| Code | Meaning |
|---|---|
| `speech_unavailable` | Speech disabled, or the feature is off for this machine (the reason is in `speech_status`) |
| `speech_engine_missing` | The worker executable was not found |
| `speech_model_missing` | The model files are absent |
| `speech_model_corrupt` | A downloaded or side-loaded file does not match its pinned size or SHA-256 |
| `speech_model_download_failed` | The download or a write failed; its data carries the model's folder, where the files can be placed by hand, then Verify (phase 3b) |
| `speech_audio_too_short` | Under 0.4 s: refused before any engine call |
| `speech_audio_too_quiet` | Below the silence floor: refused before any engine call |
| `speech_audio_too_long` | Over the cap of the setting |
| `speech_audio_invalid` | Not the canonical WAV (RIFF/WAVE, PCM, mono, 16 kHz, 16 bits, consistent length) |
| `speech_busy` | A speech job is already running |
| `speech_cancelled` | The user cancelled |
| `speech_timeout` | The worker exceeded the timeout (proportional to the audio length, with a floor) |
| `speech_engine_failed` | The worker failed or died |
| `speech_locale_unsupported` | The locale has no enabled row, or no pack for the feature |
| `speech_output_rejected` | The output guard refused the text (empty, runaway repetition, words per second out of band, output for audio flagged too quiet) |
| `tts_no_local_voice` | No voice proven local matches the locale: a refusal, never a fallback |
| `tts_failed` | Synthesis failed |
| `notes_folder_unavailable` | The Notes folder cannot be created or revalidated (policy, symlink) |
| `notes_name_invalid` | The name cannot become a clean file name |
| `notes_too_large` | A note exceeds 1 MB |
| `notes_too_many` | More than 20 files in one save |
| `notes_write_failed` | A write failed (the partial file is removed) |
| `notes_analysis_failed` | The Analyse failed after a successful save; the save is never undone |

Phase 3b added `speech_model_download_failed`, with both sentences. The model state of `speech_status` has a fourth value,
`unverified` (every file there with its pinned size, none hashed yet: a model placed by hand); dictation then reports
`speech_model_missing` and the settings offer Verify.

## 7. Flows

### 7.1 Dictation in the chat (phases 3, 4, 5, 3b, then 6)

```text
MicButton (Composer, between "@" and the model select)  -> voice session claim("composer")
  AudioCapture: 16 kHz mono 16-bit, echo cancellation / noise suppression / auto gain OFF, level callback, peak and clipped share
  stop -> canonical WAV bytes -> invoke speech_transcribe (raw body; nothing else: language and cap come from Rust)
    Rust: SpeechService takes the busy flag (drop guard) -> temp WAV in the private folder -> worker_stt (kill on cancel/timeout)
          -> output guard -> temp file removed -> { text, audio_ms, processing_ms } or a machine code
  webview: insertion.ts puts the text at the remembered selection (or appends), one-step Undo, caret restored;
           below 0.4 s or too quiet = refused before any engine call; clipped audio = transcribe plus a non-blocking warning
  user edits -> user presses Send (Enter or button). Dictation never sends.
```

States: `unavailable | idle | requesting-permission | recording | transcribing | done | error`, cancel in every state.
Send, Stop, clear and unmount cancel a dictation first. The mic is disabled when a folder is missing, while the chat is
busy, and while the voice session belongs to another owner. During an Analyse a hint says the machine is busy. A timing
line (counts and milliseconds only) goes to `speech-timings.jsonl` behind `write_timing_log`.

### 7.2 Names after a dictation (phase 6, D7)

`speech_repair_names(transcript, scope, caseReliable)` is a thin shell over `knowledge::repair::propose`, **selection
scoped, read-only, empty when `knowledgeMode` is `off` and on any failure**. The webview shows "heard -> display" chips
(Accept, Ignore, and the alternatives for an "ask"). Three guards, all required:

1. only names of the **current selection** are proposed (never an entity outside it);
2. while the hidden-names setting is on and not revealed, chips collapse to one neutral line (reuse `revealNames`);
3. a chip is only a suggestion: Accept replaces the span after checking the draft still holds `heard` there, otherwise the
   chip is dropped; nothing is replaced silently and nothing is written to the KB.

The command, the chips and the hook are generic over entity types and subtypes (a future medication entity needs no
change; no drug list ships). The chip component and the hook hold no Composer state, so the Notes editor reuses them.
Evidence: SpeechLab measured that strict dictionary correction broke 0 words, loose correction regressed, and that a
dictionary cannot tell a misspelling from a rare real word, hence "suggest, never replace". Engine hotwords and prompts
are not part of the programme (marginal or harmful in SpeechLab, and a prompt would travel on a command line).

### 7.3 Voice notes (phases 10, 11, 12, D9, D10, D14)

- **Surface.** The title-bar **Notes** button sits beside Settings (own switch `notesEnabled`). It opens the centre on the
  Notes section, or hides the zone when already there. A two-position switch **Files | Notes** at the top of the centre zone
  (shown only when `notesEnabled` is on) keeps the file tabs: both sections stay mounted, so file tabs keep their scroll,
  zoom and sheet while she writes, and an unsaved note and a recording in progress survive a switch. With the switch off
  the centre renders exactly as today.
- **Pane, like Typeless.** The text fills the pane, editable, with a Raw / Preview switch (the application's Markdown
  component for the preview); a dark pill button centred at the bottom (microphone at rest, animated waveform driven by the
  input level while recording, elapsed time, "click again to finish"); a save block under the text: name field (the
  placeholder shows the timestamp), format select (.md default, .txt), Save. A switcher for several drafts and a "new note"
  button. The pane works at the 320 px minimum.
- **Dictation into a note** (phase 12): an energy-based segmenter cuts on a silence of about 0.7 s once a segment is at least
  about 1.5 s, with a hard cap about 25 s at the quietest frame (a latency device, not an engine limit: SpeechLab saw no Parakeet
  failure up to 45.8 s of audio); segments are transcribed in order through a serial queue while she keeps speaking; Stop
  finishes the queue, Cancel discards it. The model stays loaded between segments, so a 30 min note holds 1.6 to 1.9 GB
  resident for its whole length (measured, Parakeet).
- **Voice commands** (`notesVoiceCommands`): punctuation words, "title" -> `# `, "subtitle" -> `## `, new line, new paragraph,
  new note. A title runs to the end of the sentence, a new-line command, or a pause. Command words are data in
  `dictation_commands` of the locale pack; matching is on folded text (accents and case).
- **Saving** (phase 10): `notes_preview(drafts)` lists the exact files and editable names, then `notes_save(drafts)` writes
  them, in `<documents folder>/<notesFolderName>`: file name = local timestamp (`2026-10-18_14-32-05`) then `-` and the
  user's sanitised name (ASCII, no spaces or accents), auto-numbered `-2`, `-3`, **never overwritten**; `.md` by default or `.txt`;
  UTF-8 without BOM, `\n` line ends, exactly the reviewed text, no injected header; limits 20 files and 1 MB each; each
  file created with `create_new` and removed if its write fails (whole or nothing); destination revalidated against the
  work-folder policy, symlinks refused; one log line per file (path and counts, no text). **The user's click on a preview of
  the exact files is the only way a note reaches the disk.** Voice never applies a file action.
- **Save also analyses** (D9, validated): the click is the user's act. The per-file path of the indexing is **not**
  separable (one pass of about 600 lines that deletes from the index every document not in the list it is given), so Save runs
  the existing full Analyse through the App's `indexing.run()`, the same hook as the Documents card, which shows it running
  and prevents a second pass. Unchanged files are skipped by hash, so it stays cheap when little changed. When other files are
  also pending (counted in TypeScript from the inventory), a confirmation asks first. A failed analysis never undoes the
  save (`notes_analysis_failed`). Nothing else starts an Analyse.
- **Never lose a note silently** (D9, D10): drafts live in App memory only, never on disk. A modal (Save / Discard / Cancel)
  waits for the user before an unsaved note can be lost: choosing another folder, the window close request, turning
  `notesEnabled` or the voice master switch off, "Reset" in Settings, and the explicit Discard; hiding the zone while a
  recording is running asks first (hiding unmounts the pane and would destroy the recording), otherwise it keeps the
  drafts. Switching section is never an exit path. One pure function (`lib/notesDrafts.ts`) decides "allow" or "ask".
  Quitting on macOS (menu, Cmd+Q) does not emit the window close request: phase 14 owns it.
- **D14.** The Notes folder is inside the Documents folder, so the next Analyse reads, embeds (through the practice server
  on the local network) and profiles the notes like any document. The rule "audio and transcripts never cross the network"
  applies to audio, and to dictated text until the user saves it.

### 7.4 Read-aloud (phases 7, 8, 9, D8)

- **Text.** Derived by code, in Rust, never by the model (`speakable::prepare(parts, locale, rules)`, shell
  `speech_prepare_reading`): strip Markdown, URLs and emoji; lists as sentences; small tables row by row, large tables replaced
  by a note code; paths reduced to the file base name; numbers without grouping; percent, currency and units as words;
  dates and times in the locale order; ordinals; titles through `title_spoken`; identifiers (AVS, IBAN, invoice) in digit
  groups using the knowledge identifier patterns; acronym spell-out list; an abbreviation-aware sentence splitter with a
  length cap; pauses sentence versus paragraph. Number, date, time, unit and ordinal rewriting is **always** applied, for every
  voice (Coqui css10 speaks no digit without it; SpeechLab D-043). The KB `spoken_form` step is left out unless the owner asks.
- **What is read.** The shown answer text, the engine-computed block, and the **caution notes**: without documents,
  unanalysed, scope outdated, interrupted, mixed corrections and rejected citations, unverified numbers, and the two lines that
  say which side of a mixed question could not be established (`mixedTableUnavailable`, `mixedDocumentsUnavailable`: a listener who
  does not see the screen would otherwise take the answer for complete). **Never** timings, sources, coverage, the
  `not_asked_about` variants (they only say the question did not concern that side) or `documentsNotNeeded` (informative only).
  The `lib/annotations.ts` split into `cautionLines` and the rest keeps `annotationLines` returning exactly what it returns today.
- **Button.** One `ReadAloudButton` in the idle assistant row that holds Copy and Regenerate, after Regenerate. Read aloud / Stop,
  a speaking indicator, one voice at a time. It stops on unmount (new question, regenerate, clear), on Escape when nothing else
  holds Escape, and on window close. It never starts without a click. Play and Stop only: no pause, no seek, no highlight.
- **Engine choice.** The Piper worker if installed (phase 9); else system voices through `speechSynthesis`, **only** voices
  with `localService === true` (exact language, then primary language; a preferred voice honoured; `null` when none, never a default
  or online voice) and only after the owner has proven them local (spike S4); else `tts_no_local_voice` and an explanatory
  sentence. WebView2 on Windows lists online "Natural" voices with `localService === false`, which is why the filter exists.
- **Playback.** Web Audio `decodeAudioData` on the returned WAV bytes: no `media-src`, no `blob:` in the policy. The rate is the
  synthesiser `speed`; the buffer source's `playbackRate` stays at 1.

## 8. Privacy

| Data | Where it lives | Stored? | Leaves the machine? |
|---|---|---|---|
| Microphone audio | Webview memory, then a private temp WAV in the app-local data folder while one job runs | **Never kept**: removed on success, error, cancel and timeout; a lazy sweep removes orphans at the first speech command | Never |
| Transcript of a dictation | The text box (and the conversation once sent, like typed text) | Only as typed text is | Only when the user sends the question, exactly as for typed text |
| Dictated note | App memory (D10: no recovery copy) | Only when the user clicks Save: then an ordinary document (D14) | After Save, the document follows the document path (embedding through the practice server on the LAN) |
| Answer text for read-aloud | Derived in Rust from what is on screen | No | Never; the voice engine is local |
| Names (suggestions) | Selection only, on screen | No | Never; nothing is written to the KB |
| Model files | `models` folder of the app-local data directory | Yes (that is their purpose) | Download only, on an explicit click, from `speechModelsUrl`; nothing is sent but the HTTP request |
| Logs and timing lines | `speech-timings.jsonl`, behind `write_timing_log`, and the notes log | Counts, milliseconds, paths and codes | **No text, no name, no transcript** |

Other facts: the webview never calls a worker or the server directly; workers have no network code and receive text on
stdin; a command line never carries text; no cloud speech; `/v1/audio/*` stays reserved and unused. The Content Security
Policy gains **no** `blob:` in `script-src`, `worker-src` or `media-src`: the capture worklet is a static file
(`public/speech/capture-worklet.js`, loaded from the same origin), and playback uses Web Audio. The WebView2 runtime itself
may contact a Microsoft address; that is not the application's code (same finding as in `docs/PRIVACY-AND-SECURITY.md`).
The first microphone use shows the WebView2 permission prompt for this application's origin; a refusal is a catalogue sentence
that points to the system setting. Disk encryption and the DPIA gate (`AGENTS.md`, item 6) cover the residue risk of a temp
file; real documents stay behind that gate, so every speech test uses fictional data.

## 9. Models and download (D5)

`resources/speech/models-manifest.json`, compiled in: for each model an id, display key, languages, size, **SHA-256 of the
download and of each extracted file**, licence, source URL (a path under `speechModelsUrl`), engine. Hosted as **`.zip`**
(the `zip` crate is already a dependency; no `tar` or `bzip2` crate): the owner re-packs SpeechLab's `.tar.bz2` archives
(Parakeet: 487,170,055 bytes as `.tar.bz2`, files `encoder.int8.onnx`, `decoder.int8.onnx`, `joiner.int8.onnx`, `tokens.txt`).
The exact zip names, sizes and hashes are handed to the owner by phases 3b and 9. One file, `speech/model_download.rs`, holds the
network code, on an explicit command only: progress channel, cancel, SHA-256 verification, a marker file written after
verification. A source scan proves that no other file of the module opens a connection.

Two paths, because the practice machine may be offline or behind an antivirus that intercepts TLS (SpeechLab I-009; `reqwest`
here uses `rustls-tls`, which does not read the Windows certificate store): the download, and **side-loading** (files placed by
hand in the models folder, then "Verify"). The error of a failed download says so. The gateway's shared HTTP client is not
changed. Every pulled model needs its licence row (section 10) before it is proposed.

**As built by phase 3b.** The models live in `<app local data>/speech-models/<model id>/`. The manifest pins the size and the
SHA-256 of the zip and of each of the four files; the download hashes while it writes, unpacks only the four names of the
manifest (never a path taken from the zip) into a staging folder, hashes each file, writes the marker `.verified` (a digest of
the manifest row) and replaces the model's folder. Looking at the state costs a few `stat` calls (sizes and the marker); only
Install and Verify hash. Uploaded to `https://assistantcabinet.ai/models/speech/`: `parakeet-tdt-0.6b-v3-int8.zip`, 478,123,854
bytes, SHA-256 `3781ca818590747c1ff244a8193b541ba8ea32506253832aa1a19ced0645883e`, holding the four files flat (the report lists
their sizes and digests). The fetch script for the native libraries is `scripts/fetch-speech-resources.sh` (and `.ps1`), the
build and the espeak check `scripts/build-speech-worker.sh` (and `.ps1`).

## 10. Licences

Policy: `docs/MODELS.md`, "Licence policy" (Apache 2.0 or MIT, owner decision of 7 October 2026) is the policy of the **chat
model families**. For speech, the owner's decisions D2 and D3 name each model and voice and are the explicit decision for
them; the rule stays for every other model. See "Open points", item 1 (confirmed by the owner, 18 October 2026).

`models/LICENSES.md` is under `/models/` in `.claudeignore` and was not read, so its columns are not guessed. The rows below
are the text to paste into it (the owner pastes it; columns suggested: model or component, role, licence, source, obligation,
status).

| Model or component | Role | Licence | Source | Obligation | Status |
|---|---|---|---|---|---|
| NVIDIA Parakeet TDT 0.6B v3, int8, ONNX by sherpa-onnx | Speech to text (D2) | **CC BY 4.0** | `https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3`; ONNX archive from `k2-fsa/sherpa-onnx` releases | Attribution text below, visible in the credits view; indicate that the weights are quantised to int8 and converted to ONNX | Accepted by the owner, 17 Oct 2026 (D2) |
| sherpa-onnx 1.13.x (crate and prebuilt static libraries) | Runtime of both workers | Apache-2.0 | `https://github.com/k2-fsa/sherpa-onnx` | NOTICE and licence travel with each worker | Use |
| ONNX Runtime (inside the sherpa-onnx libraries) | Runtime | MIT | via sherpa-onnx | Notice | Use |
| espeak-ng (inside the **full** sherpa-onnx libraries) | Phonemizer of the Piper voices | **GPL-3.0** | via sherpa-onnx | Only in the separate TTS worker, with its own notice file, licence text and `espeak-ng-data`; **never** in the main executable or the STT worker (post-build scan) | Legal reading of the "separate executable" position is open (item 3) |
| whisper.cpp (and ggml) | Fallback engine behind the same trait | MIT | `https://github.com/ggml-org/whisper.cpp` | Notice | Not shipped; recorded fallback |
| Piper `fr_FR-siwis-medium` | French voice, female (D3) | Data CC BY 4.0 (SIWIS); **fine-tuned from the Lessac voice**, whose data is a research-only agreement. SpeechLab rating: **`review`** | Piper voices, via the owner's host | Attribution; **licence settled before any distribution beyond the controlled prototype**; retraining from the clean base is a later track | Accepted for the controlled prototype (owner, 18 Oct 2026); development and non-commercial use |
| Coqui VITS `fr` `css10` | French voice, male (D3; gender to confirm by ear) | Weights BSD-3-Clause (Coqui model list); data CSS10 French = LibriVox public domain, repository Apache-2.0. Rating: `attribution`; no phonemizer (character based) | Coqui, via the owner's host | Notice | Accepted (D3) |
| Piper `en_US-ljspeech-medium` | English voice (D3) | Public domain (LJ Speech), trained from scratch. Rating: `clear` | Piper voices | None | Accepted |
| Piper `en_US-libritts_r-medium` | English voice (D3) | CC BY 4.0 (LibriTTS-R). Rating: `attribution` | Piper voices | Attribution | Accepted |
| Piper `fr_FR-mls-medium` | Rejected by the owner (heard, 1 to 2 out of 3) | CC BY 4.0, trained from scratch. `attribution` | - | - | **Rejected** |
| Piper `fr_FR-gilles-low` | Excluded | CC0 data, **fine-tuned from the Ryan voice** (CC BY-NC-SA). `excluded` | - | - | **Excluded**, not shipped |

Source of the ratings: SpeechLab `docs/TTS_LICENSES.md` (read 18 October 2026), whose own caveat applies: a technical reading
of public licence texts and model cards, not legal advice. The rating is shown in the voice list and in the credits view and is
never hidden. A voice is added only after its card has been read, its lineage followed one step, and its phonemizer recorded.

**Attribution text for Parakeet** (to show in the credits view, phase 13; the wording is to be checked against the model card
when the model is pulled in phase 3b):

> Speech recognition uses NVIDIA Parakeet TDT 0.6B v3 (`nvidia/parakeet-tdt-0.6b-v3`), (c) NVIDIA, licensed under the
> Creative Commons Attribution 4.0 International licence (https://creativecommons.org/licenses/by/4.0/). The weights are used
> in an int8-quantised ONNX conversion published with sherpa-onnx (k2-fsa); no other change was made.

**siwis in one paragraph.** `siwis` is the female French voice of the controlled prototype because the pilot GP is the owner's
relative and use is a controlled prototype. Its lineage rating is `review`, SpeechLab's owner decision D-043 keeps it for
development, evaluation and non-commercial use, and a commercial or wider release is blocked until it is retrained from the clean
base checkpoint (`rhasspy/piper-checkpoints`, folder `_base_model`; needs a GPU), or cleared in writing, and listened to blind
again. That retraining is a later track outside the programme's phases.

## 11. Minimum configuration

Reference machine: the owner's laptop (Intel Core 7 150U, 10 cores, 23.6 GB, NVMe, Windows 11; `docs/HARDWARE.md`, "Development
PC"), the machine SpeechLab measured, with about 20 % background load. The practice workstation (a 2019 PC) is being replaced:
**nothing is measured on the old one**. The same short check runs on the **new** workstation when it exists: free RAM, time to
transcribe a 10 s clip, CPU load, time to speak a paragraph. If it fails, whisper.cpp (MIT, behind the same trait) is the fallback
engine.

| Feature | What SpeechLab measured (owner's laptop) | Label |
|---|---|---|
| Dictation, Parakeet TDT v3 int8 | 1.6 to 1.9 GB resident; 7 to 10 busy cores although 4 threads are requested; cold load about 2 s; real-time factor about 0.10 (about 10 times faster than real time); a 5 s sentence in about 0.5 s; word error rate 2.3 % overall, 2.1 % in French, one speaker; 487 MB model download | measured |
| Dictation, the worker of phase 3b on the same laptop (18 Oct 2026, release build, 4 threads requested) | worker process alone: 733 MB resident after the model load, 857 MB peak after decoding (commit 864 MB); cold load 2.2 s (2.5 s and, once with a cold file cache, 4.3 s); a 10 s clip in 0.9 to 1.1 s, about 7 busy cores; the cancel of a decode returns in 0.6 s and leaves no process; 18.6 MB executable, no `espeak` in it | measured |
| Read-aloud, Piper voices (siwis, ljspeech, libritts_r) | 200 to 300 MB resident; about 67 to 82 MB on disk; real-time factor 0.045 to 0.064; cold load 0.8 to 1.3 s | measured |
| Read-aloud, Coqui css10 | 231 MB resident; 67 MB on disk; needs the number rewriting for digits | measured |
| Notes | Dictation figures, for the whole length of the note: the model stays loaded (idle unload about 120 s), so a 30 min note holds 1.6 to 1.9 GB resident throughout; during a decode the machine is busy, including during an Analyse | measured (derived) |

| Computer | Dictation | Read-aloud | Notes | Label |
|---|---|---|---|---|
| **Minimum** (no more than) | 4 cores, 8 GB RAM of which 3 GB free for the worker, SSD, 1 GB free disk for models; a 10 s clip within about 10 s | 2 GB RAM free; one voice installed | As dictation; a 30 min note may keep the machine slow while it is decoded | **estimate** |
| **Recommended** | 6 or more cores, 16 GB RAM, NVMe, nothing else heavy running during a dictation | As the minimum | As dictation | **estimate** |

The two rows above are estimates reasoned from the laptop's figures; they are replaced by measurements of the new workstation at
phases 3b (engine), 9 (voices) and 13 (final check). Phases 3b and 9 add their own measured figures here, labelled measured;
phase 13 finalises the section, or leaves a figure labelled estimate with the reason.

## 12. Invariants, each proved by a named test

| Invariant (from the decisions) | Named test (suggested; the phase names the final one) | Phase |
|---|---|---|
| Audio is never kept | `temp_audio_is_removed_on_success_error_cancel_and_timeout`, `startup_sweep_removes_orphan_audio` | 3 |
| No network from the module except the model download file | `no_http_client_exists_in_the_speech_module` (source scan, allows `model_download.rs` only), `the_download_file_sends_nothing_but_a_get_for_the_archive`, `the_worker_crate_contains_no_network_code`; the offline proof at the gate | 3, 3b, 13 |
| A transcript only fills a text box | `dictation_never_calls_send` | 5 |
| Voice never applies a file action | `no_speech_path_reaches_a_file_action`; `notes_save_writes_only_what_the_preview_listed` | 5, 10 |
| Dictation never writes the KB | `dictating_does_not_change_the_knowledge_base` (row hash of the `kb_*` tables), `repair_never_leaves_the_selection`, `kb_off_gives_no_repair` | 6 |
| No text in logs | `speech_timing_line_holds_no_text`, `notes_log_line_holds_no_text` | 3, 5, 10 |
| Only proven-local voices | `a_non_local_voice_is_never_selected`, `no_local_voice_refuses` | 8 |
| Spoken text derived by code, never by the model | `speakable_text_is_not_model_facing` (source scan: no gateway import) and a neutrality guard on any model-facing constant | 7 |
| Any failure degrades to the typed app | `missing_engine_reports_unavailable_and_typed_app_still_works` | 2, 3 |
| Cancel at every stage | `through_the_service_a_cancel_leaves_no_clip_and_no_process`, `cancel_kills_the_child_promptly`, the dictation state machine tests | 3b, 5 |
| One speech job at a time | `a_second_job_is_refused_with_speech_busy`; the voice session tests | 3, 4 |
| Workers die with the app | `the_worker_ends_by_itself_when_the_application_that_started_it_is_gone`, `shutdown_ends_the_worker_and_dropping_the_provider_kills_it` | 3b |
| Nothing heavy at launch | `launch_starts_no_speech_work` | 2 |
| Voice off changes nothing (D13) | `voice_off_leaves_the_existing_components_unchanged` (baseline render tests written before the first edit of phase 2) | 2, 11 |
| Language contract | the existing `sources.test.ts` and `catalogues.test.ts` guards; `a_locale_without_a_pack_answers_speech_locale_unsupported` | every phase |

Test method: unit tests on pure functions; a deterministic **fake provider** drives every integration test and runs on both CI
platforms; real-engine tests are `#[ignore]` (they need a worker and a model); vitest for the state machines and logic in
`lib/*.test.ts` and `renderToStaticMarkup` for components (there is no DOM test environment); human tests are scripts
(`AGENTS.md`, "Human tests") played on the owner's two test folders and her own voice, never real documents. Window close and
dragging belong to the human test.

## 13. Phases and their order

Branch of each phase: from `feat/speech-integration`, merged back by pull request when the owner has accepted it. File = the
handoff file in `docs/SESSION-SPEECH/`. Order and dependencies: `docs/SESSION-SPEECH/00_LISEZMOI_ordre.md`.

| Order | Phase | Branch | Needs | Gives |
|---|---|---|---|---|
| 01 | 1 Contracts and decisions | `feat/speech-contracts` | - | This specification and the decisions (documents only) |
| 02 | 2 Foundations (light) | `feat/speech-foundations` | 1 | `speech` module, settings, codes, `speech_status`, baseline tests |
| 03 | 4 Voice settings and audio capture | `feat/speech-settings-capture` | 2 | Settings block, recorder, WAV, voice session |
| 04a | 10 Notes: files backend | `feat/speech-notes-backend` | 2 | Preview and save of notes |
| 04b | 3 STT port and fake provider | `feat/speech-stt-port` | 2 | Contract, temp audio, output guard, fake engine |
| 05a | 11 Notes mode (typed) | `feat/speech-notes-mode` | 2, 10 | Notes section, drafts, save, exit guard |
| 05b | 3b STT worker and model download | `feat/speech-stt-worker` | 3 | Parakeet worker, download |
| 06 | 7 Read-aloud text preparation | `feat/speech-readaloud-text` | 2 | Speakable segments |
| 07 | 8 Read-aloud button | `feat/speech-readaloud-button` | 4, 7 | Button, system voices if proven local |
| 08 | 5 Dictation in the chat | `feat/speech-dictation` | 3, 4 (3b for the real engine) | Mic in the Composer |
| 09 | 6 Name suggestions | `feat/speech-name-suggestions` | 5 | "heard -> display" chips |
| 10 | 12 Notes dictation | `feat/speech-notes-dictation` | 3b, 4, 5, 11 | Long dictation, voice commands |
| 11 | 9 Piper voice worker | `feat/speech-piper` | 3b, 8 | Quality voice |
| 12 | 13 Prove it | `feat/speech-prove-it` | the chosen ones | Offline proof, packaging gate, credits |
| 13 | 14 macOS adaptation | `feat/speech-macos` | the Mac (November) | macOS |
| 14 | 15 German and Italian | `feat/speech-de-it` | 9, 12 | Second stage, not scheduled |

Smallest useful paths: dictation 01, 02, 03, 04b, 05b, 08, then 09; read-aloud 01, 02, 03, 06, 07, then 11; typed notes
01, 02, 04a, 05a, voice later 10.

Spikes the phases must answer and report: **S1** the static worklet under the Content Security Policy, in dev and in a release
build (phase 4); **S2** raw request body for the WAV (answered: `tauri::ipc::Request` with `InvokeBody::Raw`, proven in
SpeechLab); **S3** a `tauri.windows.conf.json` overlay declaring the sidecar on Windows only (phase 3b; answered 18 Oct 2026: yes, the overlay is read on Windows, a missing sidecar named in it stops `tauri-build` with "resource path ... doesn't exist"; the build script does not watch a newly created overlay, so `build.rs` must be touched once; not added, because the check also fails every CI run that has not staged the worker; macOS ignoring it is Tauri's documented behaviour and waits for the Mac); **S4** the owner lists the
voices of the webview and speaks a French and an English paragraph with the firewall blocking the application (phase 8);
**S5** a spawned worker flashing a console window on Windows (phase 3b; answered 18 Oct 2026: a console program started by a
`windows_subsystem = "windows"` program without a flag gets a console window of its own, measured with `GetConsoleWindow`;
with `CREATE_NO_WINDOW` it gets none. The flag is in `speech/console_window.rs`, a paired platform module, never in
`worker_stt.rs`; the Tesseract spawn in `ocr/tesseract.rs` sets no flag and has the same flash, reported and not fixed).

Packaging: there is no installer wiring for any sidecar yet (`tauri.conf.json` has no `externalBin` or `resources`; the Tesseract
block waits in `src-tauri/binaries/README.md`). Packaging the workers is the same unresolved item as packaging Tesseract: phase 13
coordinates with that track and invents no second mechanism. Until then voice works in `tauri dev` and in a locally staged build.

## 14. Open points

For the owner. **Answered on 18 October 2026:** the owner confirmed point 1 explicitly and accepted every other recommended default (including the choices of phase 1: no `cancel` message in the worker protocol, kill instead; roadmap steps V-A to V-F). The defaults below are therefore the working decisions; a point is reopened only by the owner. Points 9 and 10 still wait for a measurement or her ear, not for a decision.

| # | Point | Recommended default |
|---|---|---|
| 1 | Does D3 also settle the **licences** of the voices (css10 BSD-3-Clause, libritts_r CC BY 4.0, siwis `review`) against the free-licence policy of `docs/MODELS.md`, which names Apache 2.0 and MIT for the chat families? | Yes: D2 and D3 name them, the policy stays for chat models; the rating stays visible in the interface |
| 2 | The licence of the application's own code and the credits view (SpeechLab's Q-09) | Credits view built in phase 13; the licence of the code is decided before any distribution |
| 3 | The GPL-3.0 position of espeak-ng shipped inside a separate TTS executable (aggregation, own notice, own data folder); SpeechLab's open question 1 | Keep it separate; a legal reading before any distribution beyond the prototype |
| 4 | Where exactly the model archives are hosted (the path under `https://assistantcabinet.ai/models/speech/`) | The path is the setting `speechModelsUrl`; she corrects it if needed; phases 3b and 9 hand her the file names and hashes |
| 5 | Baseline tests of phase 2: inline expectations in one new test file (recommended) or vitest snapshot files (new committed files)? | Inline expectations: the repository has no snapshot tests |
| 6 | Add `pulldown-cmark` (phase 7) or write a line-based Markdown stripper? | A small line-based stripper; no new crate |
| 7 | Edit `.github/workflows/ci.yml` to add a `cargo check --no-default-features` step for the worker crate? | Asked in phase 3b, not edited: it waits for her answer. The check was run locally and passes (`cargo check --no-default-features --offline` in `apps/speech-worker`); the sherpa download is too heavy for the runners |
| 8 | Read the KB `spoken_form` of names of the selection in read-aloud (phase 7 second step) | Left out unless she asks |
| 9 | Whether css10 really is a male voice | Her ear, phase 9; only the label key changes |
| 10 | The new workstation's measurements and the acceptance budgets (SpeechLab's Q-08) | Section 11 estimates stay labelled until measured |

## 15. Facts verified on 18 October 2026

Read in the code of `feat/speech-integration`: three resizable zones in `AppLayout.tsx` (Resources 240, Workspace 320, Chat 340
minimum pixels); `WorkspaceView` in `lib/workspaceView.ts` has the kinds `welcome` and `files` (a list of tabs); the Content Security
Policy is `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src
'self' data: blob:; connect-src 'self' ipc: http://ipc.localhost` (`blob:` only for images); `capabilities/default.json` holds
`core:default`, `dialog:allow-open`, `clipboard-manager:allow-write-text`; `apps/desktop/public/` does not exist;
`resources/` holds `knowledge`, `pdfium`, `profile`, `tabular-questions`, `tessdata`, `tesseract`, `work-folder-questions`; no
`toMatchSnapshot` anywhere; `open_index`, `work_folder_policy`, `resolve_selection` and `sidecar_search_roots` are private in
`commands.rs`; there are two catalogues (`fr-FR`, `en-US`) and German and Italian have none. The knowledge phonetic keys exist
for French and English only.
