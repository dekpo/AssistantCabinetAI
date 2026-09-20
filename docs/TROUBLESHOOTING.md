# Troubleshooting log

One place to check before losing an hour to something another agent already hit. Append an entry every
time a non-obvious bug, footgun or environment quirk costs real time to diagnose — not every error, only
the ones a future agent (or Elise, on another machine) would otherwise rediscover from scratch. Newest
entry at the top. Keep each entry to what the next reader needs: what broke, why, and the fix, in English,
with the exact command or code where it matters.

---

## `tauri-build`'s build script validates `externalBin`/`resources` on *every* `cargo build`, not only `cargo tauri build`

**Found:** 20 September 2026, wiring the Tesseract sidecar (Sprint 2.5).

Declaring `bundle.externalBin` or `bundle.resources` in `tauri.conf.json` before the referenced files
exist breaks `cargo build` and `cargo test --lib` immediately, with an error like:

```text
resource path `binaries\tesseract-x86_64-pc-windows-msvc.exe` doesn't exist
```

This is not limited to `cargo tauri build` or to bundling — `tauri-build`'s `build.rs` reads
`tauri.conf.json` and checks every declared path on any `cargo` invocation that touches the crate. A
config change that looks purely packaging-related can break plain development and CI.

**Fix / rule:** never commit an `externalBin`/`resources` entry for a file that is not committed or not
guaranteed to exist on every machine that will run `cargo build`/`cargo test` (including CI). If the
files are fetched by a script rather than committed (see the next entry), wire the config in the same
change that also updates CI to run that fetch script first — never before. See
`apps/desktop/src-tauri/binaries/README.md` for the concrete example.

## Model weights and bundled third-party engine binaries do not go in git, even as "resources"

**Found:** 20 September 2026, same session.

`AGENTS.md`'s git rule forbids committing model weights, and `.gitignore` already excludes
`models/*.gguf` for that reason. It is tempting to treat a bundled OCR engine's traineddata, DLLs and
shared library as ordinary "resources" that ship with the app and therefore belong in the repo next to
`tauri.conf.json`. They do not: `fra.traineddata` is a model weight in the same sense, and the engine
binaries are the same class of large, non-authored, third-party blob. Nearly staged ~150 MB of binaries
into a commit before catching this.

**Fix / rule:** anything fetched from a third party rather than authored in this repo — a model weight,
a compiled engine, a prebuilt library — is gitignored and fetched by a script
(`scripts/fetch-ocr-resources.ps1` is the first example) run once per machine, the same pattern already
used for Ollama's own weights (`.ollama/`, `models/*.gguf`).

## Windows DLL search order: a spawned sidecar only searches its own directory, not `resources/`

**Found:** 20 September 2026, same session.

A Tauri sidecar's runtime DLLs must sit in the **same directory as the sidecar executable** once
bundled. Windows resolves a spawned process's DLL dependencies by searching that process's own directory
(plus `PATH` and the system directories) — it does not know or care about Tauri's `resources/` tree.
Bundling the DLLs under `resources/tesseract/` produced a sidecar that failed to start
(`STATUS_DLL_NOT_FOUND`, exit code `-1073741515`) with no readable error message, because the OS loader
error never reaches the spawning process's stdout/stderr.

**Fix:** map the sidecar's own dependency DLLs to the bundle root (`"./"` in `tauri.conf.json`'s
`bundle.resources`), not to a subfolder. A library the *application* loads directly through an explicit
path (like `pdfium.dll`, loaded via `Pdfium::bind_to_library`) has no such constraint and can live
anywhere under `resources/`.

---

## How to use this file

- New entry at the top, dated, with a short heading naming the symptom or the wrong assumption — not
  the fix — so a search for the error text finds it.
- State what actually happened (the exact error text helps), why, and the concrete fix or rule that
  prevents it. Link to the file where the real fix lives rather than duplicating code here.
- This file is versioned (`docs/`), like the rest of the English specification: read `AGENTS.md`.
