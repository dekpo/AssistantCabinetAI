# Release notes: local voice (dictation, read-aloud, voice notes)

Release candidate of 19 October 2026. **Windows: validated** on the owner's laptop (build, install, use, connection sampling).
**macOS: pending** (phase 14, a Mac is available in November). The installer is a test build for fictional data; handing it to the
pilot stays gated by `AGENTS.md`, item 6 (DPIA draft, named data controller, disk encryption).

## What is in it

- **Dictation in the chat.** A microphone button in the question box. The text only fills the box; the user reads it and sends it.
- **Name suggestions after a dictation.** Names of the current selection only, never replaced without a click, nothing written to the
  knowledge base.
- **Read-aloud.** A button under each finished answer. French voices `siwis` (female) and `css10` (male), English `ljspeech` and
  `libritts_r`, or the operating system's local voices (opt-in).
- **Voice notes.** A Notes section of the centre zone: long dictation with spoken punctuation and titles, saved only by the user's click
  on a preview of the exact files, then analysed like any document.
- **Credits and licences** at the end of the voice settings.

Everything runs on the workstation. The speech models are downloaded on an explicit click from the owner's host (`speechModelsUrl`);
that download is the only network code of the speech module.

## What the installer contains

The application, the dictation worker, the voice worker (with its espeak-ng notice) and the Tesseract sidecar, built with
`scripts/build-installer.sh`. No model and no voice: they are installed from the settings. Build of 19 October 2026: 54,054,277 bytes,
39 files, 213 MB installed. The workers need no Visual C++ Redistributable (static C runtime).

## Known limits

- **Windows only**, and not run on a clean machine (none was available): the import tables show no dependency outside Windows itself.
- **Unsigned** installer and executables: SmartScreen and some antivirus products will warn. Code signing is not decided.
- **The system web view** keeps one HTTPS connection to a Microsoft address while the application runs. It is the system component, not
  the application's code (the same finding as in SpeechLab, I-066); the workers open none.
- **Voice `siwis` is a prototype voice**: its licence must be settled, or the voice retrained, before any distribution beyond the
  controlled prototype. The settings and the credits view say so.
- **espeak-ng (GPL-3.0)** lives in the voice worker only, as a separate program with its own notice; a legal reading of that position is
  open before any distribution.
- **Measurements for the new practice workstation** are still to take; the minimum configuration in `docs/SPEECH.md` is an estimate.
- The first start of freshly installed programs can be slow (about 30 s seen once, with an antivirus present).

## Not in this release

macOS; German and Italian (phase 15); cloud speech; spoken conversation; a voice that applies a file action.
