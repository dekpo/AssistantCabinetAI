# Building and installing the Windows installer

How to produce the installer of Assistant Cabinet AI on Windows, install it on another computer, and set that computer up. Written
19 October 2026 from the build that was actually run (speech programme, phase 13). Commands are for **Git Bash**. Every command is
run from the repository root unless it says otherwise: `cd /c/Users/elise/Documents/CURSOR/AssistantCabinetAI`.

**Scope.** Test installers for fictional data, on machines the owner controls. Handing an installer to the pilot stays gated by
`AGENTS.md`, item 6 (written DPIA draft, named data controller, disk encryption on both machines). macOS is not covered
(`docs/SESSION-SPEECH` phase 14, a Mac in November).

## 0. What is validated and what is not

| Step | State |
|---|---|
| Build on the owner's development laptop (parts 2 to 4) | **Done twice** on 19 October 2026; the second build is the reference |
| Silent install into a folder, files in place (part 5) | **Done**, 7 s, no administrator rights |
| The installed application: window, dictation, read-aloud, notes available | **Done** on the building laptop (driven through the web view debugging port) |
| Install by double-click on another computer, first start there (part 6) | **Not done yet**: this document is the procedure for doing it |
| A computer without developer tools (Visual C++ runtime) | Import tables show no dependency outside Windows itself; **never run on such a machine** |
| The WebView2 runtime on a machine that lacks it (Windows 10) | **Not verified**; the Tauri default is to download its installer, which needs the Internet |
| Build on a fresh computer, or on a GitHub runner | **Not done**: `installer-windows.yml` has never run |
| Code signing | **No**: Windows SmartScreen will warn |

When you test on another machine, record what you see in `docs/test-reports/speech-programme/` (or tell the agent): it closes the
"not done" rows above.

## 1. What you get

One file, `apps/desktop/src-tauri/target/release/bundle/nsis/Assistant Cabinet AI_<version>_x64-setup.exe` (about 54 MB). It installs
**per user** (no administrator rights): the application, the dictation worker, the voice worker with its espeak-ng notice, Tesseract
(OCR) with its libraries and the French model, and pdfium. It does **not** contain a speech model or a voice: each machine downloads
them once from the settings (part 6).

Which programs go in is decided in one place, `apps/desktop/src-tauri/tauri.bundle.conf.json` (`bundle.externalBin` and
`bundle.resources`). The entries are deliberately not in `tauri.conf.json` (`docs/DECISIONS.md`, "How it is wired"): that file
stays buildable and testable without any staged program.

## 2. What the building computer needs (once)

- Windows 11, with **Git for Windows** (Git Bash, `curl`, `tar` with bzip2) and **PowerShell** (one script below uses it).
- **Rust**, stable, MSVC toolchain (`rustc -vV` says `host: x86_64-pc-windows-msvc`), with the **Visual Studio Build Tools** (C++).
- **Node 20 or newer** and **pnpm** (CI uses pnpm 12). Then `cd apps/desktop && pnpm install --frozen-lockfile`.
- **Internet** for the first build: the Tauri tool downloads NSIS the first time it makes an installer (it keeps it in
  `%LOCALAPPDATA%\tauri`).
- Disk: the Rust build folders are large (several GB). Use `-j 2` for `cargo test` if Windows reports error 1455 (paging file).
- The application must not be running while you build (it locks its own executable).

## 3. Stage the third-party programs (once, and again after a version bump)

These are never committed to git (`.gitignore`): they are large and reproducible.

```bash
bash scripts/fetch-speech-resources.sh
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-ocr-resources.ps1
```

- The first downloads the two sherpa-onnx static libraries (checked against pinned SHA-256) into `apps/speech-worker/vendor/`.
- The second installs Tesseract silently (Windows may show an administrator prompt), copies the programs it needs and downloads
  pdfium and `fra.traineddata` into `apps/desktop/src-tauri/binaries/` and `.../resources/`.

## 4. Build the installer

```bash
bash scripts/build-installer.sh --bundles nsis
```

What it does, in order (it stops at the first problem and says what is missing):

1. reads the list of programs from `tauri.bundle.conf.json`;
2. builds `speech-stt-worker` and `speech-tts-worker` in release mode (static C runtime, so no Visual C++ Redistributable is needed),
   checks that the voice worker holds espeak-ng and the dictation worker does not, and stages both in `binaries/`;
3. checks that Tesseract and every file of `bundle.resources` are staged;
4. runs `pnpm tauri build --config src-tauri/tauri.bundle.conf.json --bundles nsis` (TypeScript build, release build of the
   application, NSIS).

Time seen on the owner's laptop with a built tree: workers about 1 minute each, application about 2.5 minutes, install 7 s. A first
build on a computer that never built the project will take much longer (not measured; plan 15 to 30 minutes).

When it ends, note the size and the checksum of the file, to compare on the other machine:

```bash
sha256sum "apps/desktop/src-tauri/target/release/bundle/nsis/Assistant Cabinet AI_0.1.0_x64-setup.exe"
```

The file name follows `version` in `apps/desktop/src-tauri/tauri.conf.json`. To number a new test build, change it there (and keep
`apps/desktop/package.json` and `src-tauri/Cargo.toml` equal), then rebuild.

## 5. Install

**On another computer.** Copy the `.exe` (USB key or a shared folder on the home network), check its checksum
(`sha256sum` in Git Bash, or `Get-FileHash` in PowerShell, against the value of part 4), then double-click it. The installer is not
signed, so Windows may show "Windows protected your PC": choose "More info", then "Run anyway". Some antivirus products scan or
block a new unsigned program; if one does, allow it for the test and note which product it was. The install needs no administrator
rights. The default folder is under the user's local programs folder (not checked: the tests used the explicit folder below).

**Silent install into a chosen folder** (tests, scripts; this is the form that was run):

```bash
MSYS_NO_PATHCONV=1 "/c/path/to/Assistant Cabinet AI_0.1.0_x64-setup.exe" /S /D='C:\Users\elise\AcaiProof'
```

Wait about 10 seconds. It creates a Desktop shortcut and a Start Menu shortcut, an entry in "Installed apps", and the registry key
`HKCU\Software\assistantcabinetai`.

**Update.** Run the newer installer over the old one. The user's settings, index and models live elsewhere (below) and are kept.

**Where the user's data lives** (not touched by installing): settings in `%APPDATA%\com.assistantcabinetai.desktop\`, index, logs
and speech models in `%LOCALAPPDATA%\com.assistantcabinetai.desktop\`. These are shared by every copy of the application on that
Windows user, including a development build.

**Remove.** "Installed apps", Assistant Cabinet AI, Uninstall; leave the "delete the application data" box **unticked** unless you
mean it. Do not run the uninstaller on a machine whose data you care about without having saved those two folders first (the silent
uninstaller was seen to keep user data in SpeechLab; this application's uninstaller was never run). To remove a throwaway test copy
by hand instead, see the last step of `docs/SESSION-SPEECH/HUMAN-TEST-13-offline-proof.md` (folder, uninstall key, registry key,
two shortcuts).

## 6. Set up each computer

The installed application starts with defaults: voice off, no folder, no model.

1. **The AI.** Réglages, "Adresse de l'IA": the address of the practice server on the home network, for example
   `http://192.168.1.83:8080` (the address is a setting, never a constant). That machine must be on and reachable from this one
   (same network, its firewall open on that port).
2. **The folders.** Choose the documents folder and the data folder. Use fictional documents only (`AGENTS.md`, item 6).
3. **The voice.** Réglages, bloc "Voix": tick "Utiliser la voix". Press "Installer" for the speech model (about 478 MB) and for the
   voice of your language (about 65 to 82 MB each). They download from `https://assistantcabinet.ai/models/speech/` (needs the
   Internet, once; a long download sometimes stops: press Install again). Windows asks once for the microphone for this application.
4. **Check.** Réglages, "Voix", "Crédits et licences de la voix" lists the components and the voices. Try the microphone in the
   question box, and "Lire à voix haute" under an answer.
5. Minimum computer for the voice: `docs/SPEECH.md`, section 11 (an estimate: 4 cores, 8 GB of RAM with 3 GB free, SSD).

## 7. When something goes wrong

| What you see | What it means | What to do |
|---|---|---|
| `missing ... stage it first` or `missing resource ...` from the script | Part 3 was not done on this computer | Run part 3, then part 4 again |
| `checksum mismatch` while fetching | Corrupted or intercepted download | Run the fetch script again; behind an antivirus that reads HTTPS, download by hand and check the pinned SHA-256 |
| `resource path ... doesn't exist` on a plain `cargo test` | Someone put the packaging entries in `tauri.conf.json` | Move them back to `tauri.bundle.conf.json` |
| `os error 5` or `32` while building | The application is still running | Close it, then rebuild |
| `error 1455`, or the machine freezes on `cargo test` | Paging file too small | `cargo test -j 2`; socket tests one at a time (`docs/TROUBLESHOOTING.md`) |
| NSIS cannot be downloaded | No Internet for the Tauri tool | Connect once; it is cached afterwards |
| The microphone does nothing | Windows blocked it for desktop apps | Windows Settings, Privacy, Microphone; then the prompt of the application |
| "Aucune réponse de l'IA à l'adresse ..." | The server is off, on another network, or its address is wrong | Check "Adresse de l'IA" and the server |
| Dictation or reading says the component or the model is missing | The model or voice is not installed, or the program was removed by an antivirus | Settings, "Voix": Verify, then Install; look in the antivirus quarantine |

## 8. Offline proof and tools

`scripts/speech-offline-rule.sh` (firewall rule that blocks every program of an installed copy; Git Bash as administrator) and
`scripts/speech-connection-sample.mjs` (lists the connections of the application's process tree) prove that the voice works with the
network blocked. Procedure: `docs/SESSION-SPEECH/HUMAN-TEST-13-offline-proof.md` (local). Method and results: `docs/SPEECH.md`,
section 16.

## 9. Known limits of this method

Windows only; unsigned; no clean-machine run; no CI run; the WebView2 runtime is assumed present on Windows 11; the first start of
freshly installed programs can take about 30 seconds with an antivirus; the licences of the voices (`siwis` is a prototype voice) and
of espeak-ng in a separate program must be settled before any distribution beyond the controlled prototype
(`docs/SPEECH.md`, section 10).
