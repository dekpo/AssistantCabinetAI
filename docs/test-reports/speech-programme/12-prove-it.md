# Speech 13: Prove it (lot report)

Branch `feat/speech-prove-it`, 19 October 2026. Windows only; macOS waits for phase 14.

## What was built

| Piece | Where |
|---|---|
| One list of bundled programs and files | `apps/desktop/src-tauri/tauri.bundle.conf.json` (`externalBin`: both workers and Tesseract; `resources`: voice notice, Tesseract libraries, `fra.traineddata`, `pdfium.dll`) |
| Installer build | `scripts/build-installer.sh` (reads the list, builds the two workers, checks the staged files, runs `tauri build --config`) |
| Manual CI workflow, Windows | `.github/workflows/installer-windows.yml` (never run on a runner) |
| Static C runtime for both workers | one line in `scripts/build-speech-worker.sh` and `scripts/build-speech-tts-worker.sh` |
| Tests of names and places | `speech/packaging_tests.rs` (6 tests) |
| Missing invariant test | `speech/proof_tests.rs`: `no_speech_path_reaches_a_file_action` |
| Credits view | `components/SpeechCredits.tsx`, `speech/credits.ts`, `speech.credits.*` in both catalogues, `speech/credits.test.ts` (6 tests) |
| Offline proof tooling | `scripts/speech-offline-rule.sh` (firewall rule, Git Bash as administrator), `scripts/speech-connection-sample.mjs` (connections of the process tree) |
| Docs | `docs/SPEECH.md` (section 12 names corrected, section 11 and 16, packaging line), `docs/DECISIONS.md` ("How it is wired"), `docs/RELEASE-NOTES-SPEECH.md` |

**Deviation from the decision of 19 October, with the owner's choice (asked, answered "separate file"):** the bundle entries are in
`tauri.bundle.conf.json`, not in `tauri.conf.json`, because Tauri checks every `externalBin` file at every build and `cargo test` would
stop on any machine without the staged programs (SpeechLab I-064; the old `binaries/README.md` of this repository). One mechanism for the
three programs, as decided.

## Edits to existing files

`commands.rs`: `sidecar_search_roots` widened to `pub(crate)` (one word). `speech/mod.rs`: two `mod` lines (tests). `SpeechSettings.tsx`:
one import and `<SpeechCredits status={status} />` at the end of the block. `speechSettings.render.test.ts`: the count of `<details>` is
2 (the voice block and its closed credits block) instead of 1. `en-US.json`, `fr-FR.json`: the `speech.credits` group. The two worker
build scripts: the static-runtime flag. Not widened: `open_index`, `resolve_selection`, `work_folder_policy` (already `pub(crate)`),
`AppState::read`, `AppState::vocabulary`.

## Verified and measured

- **Final pass (once, after the last edit):** `cargo test --lib --tests --no-fail-fast -j 2` all green (library 1098 passed, 9 ignored;
  every integration target ok); `cargo clippy --lib --tests`: no warning in the files of this phase (the existing warnings of other test
  targets are unchanged); `pnpm run build` ok; `pnpm run test` 93 files, 2244 tests passed. New Rust files `rustfmt --check` clean.
  Pre-check before the first edit: `sidecar_discovery` 1 of 1.
- **Installer, built twice (the second after the static-runtime change):** 54,054,277 bytes, SHA-256
  `B1042B730748BE3ABCF7AE5B916A87D3EC48B728A871EAA7E1BA3E26890A3773`; compile of the application about 2 min 30 s on top of a built tree.
  Silent install (`/S /D=`) in 7 s: 39 files, 213,330,601 bytes. Layout: both workers, the notice, `tesseract.exe` and its libraries at
  the install root, `resources\tessdata\fra.traineddata`, `resources\pdfium\pdfium.dll`. No model, voice or `espeak-ng-data`.
- **Installed application, driven through the web view debugging port:** window ready 0.56 s after the process start; `speech_status`:
  voice on, dictation, read-aloud and notes available, the recognition model and the four voices present; a French sentence spoken in
  0.95 s (95,788 bytes of audio) and read back by the recogniser in 2.66 s as "Bonjour, ceci est un essai de la voix." (word for word).
  The first run of the freshly installed programs, with the sampler running, took 28.7 s and 30.1 s for the same two calls: not
  separated (new unsigned executables scanned by the antivirus, and the sampler's own load).
- **Connections, installed application, 40 to 45 s, all three features used, no firewall rule:** the workers: none. The application:
  one connection to `192.168.1.83:8080` (the practice server; purpose not traced). The system web view: one HTTPS connection to a
  Microsoft address (SpeechLab I-066 again). Idle for 45 s: the same.
- **Visual C++ runtime (I-070):** import tables read from the installed files. Application and Tesseract: no Visual C++ runtime. Both
  workers as first built: `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`. After `-C target-feature=+crt-static`: Windows system libraries
  only; the rebuilt workers worked in the installed application. **Not run on a clean machine** (none available; Windows Home has no
  Sandbox).
- **Source scans and tests of the invariants:** table of `docs/SPEECH.md` section 12 now names tests that exist (nine of its old names
  did not); `no_speech_path_reaches_a_file_action` added.
- **Cleanup of the test installs:** the first throwaway install was removed by hand (folder, uninstall key,
  `HKCU\Software\assistantcabinetai`, two shortcuts), the uninstaller was not run. The second install, `C:\Users\elise\AcaiProof`, is
  **left in place for the owner's human test** (below), with its shortcuts and keys.

## Not verified (said plainly)

The offline proof **with the firewall rule on** (needs an administrator, so it is the owner's part: see the test below); the interactive
uninstaller; a clean machine; code signing and SmartScreen; macOS; the new workstation's measurements (the "Minimum configuration" rows
stay **estimate**, with the reason in `docs/SPEECH.md` section 11); the Windows workflow on a GitHub runner; `ci.yml` on both platforms
(unchanged by this phase, its result comes with the pull request).

## Invariants and evidence

See `docs/SPEECH.md`, section 12 (names checked against the code on 19 October 2026). Evidence added here: the packaged application
used all three features with no connection from either worker; the offline row completes when the owner plays the proof.

## Owner decisions or inputs still open

Open point 10 (new workstation figures); the siwis licence track; legal reading of espeak-ng in a separate executable; code signing;
`models/LICENSES.md` rows (section 10 of `docs/SPEECH.md`, `.claudeignore`); whether to keep `installer-windows.yml`.

## Prompt alignment

Checked `13_speech-14_macos-adaptation.md`: it assumed packaging was still to be invented and named private helpers; appended "Alignment
with phase 13" (one config file, Windows-only script, static runtime, `pub(crate)` finder root, Windows-only proof tooling). The
German/Italian prompt is not touched by this phase. `STATUS.md` updated.
