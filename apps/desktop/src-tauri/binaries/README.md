# Tesseract sidecar binaries

Not committed, and not committable: `AGENTS.md` and `.cursor/rules/git-workflow.mdc` forbid model
weights in git, and a bundled OCR engine is the same kind of large, reproducible third-party
binary as `.ollama/`'s own weights (`.gitignore` already excludes `models/*.gguf` for the identical
reason). `fra.traineddata` is a model weight in the same sense; the Tesseract executable and its
runtime DLLs are the engine that reads it.

Run `scripts/fetch-ocr-resources.ps1` from the repository root before building. It downloads the
UB-Mannheim Tesseract 5 Windows installer, installs it, and copies `tesseract.exe` here as
`tesseract-x86_64-pc-windows-msvc.exe` - the name Tauri 2's `externalBin` convention expects for
this target triple - plus the DLLs it actually loads into `../resources/tesseract/`.

```text
binaries/tesseract-x86_64-pc-windows-msvc.exe   fetched by the script above
binaries/tesseract-aarch64-apple-darwin         not yet automated - see docs/SPRINT-2.5-ASSESSMENT.md section L
binaries/tesseract-x86_64-apple-darwin          not yet automated
```

**Verified in this session, on this Windows machine, then deliberately not wired into
`tauri.conf.json` yet - read to the end before re-adding `bundle.externalBin`.** Trial removal
against the full DLL set the installer bundles shows `tesseract.exe` needs only the thirty DLLs
the script copies - none of the `libpango*`, `libcairo*`, `libglib*`, `libgio*`, `libgobject*`,
`libgmodule*`, `libicu*75.dll`, `libfontconfig*`, `libfreetype*`, `libfribidi*`, `libgraphite2*`,
`libharfbuzz*`, `libthai*`, `libdatrie*` or `libpixman*` the installer also ships, all of which
belong to its `text2image` tool. Running `tesseract.exe --version` and
`tesseract.exe <image> - -l fra tsv` against a real fixture
(`fixtures/gp-sandbox/inbox/2026-03-26_ordonnance-scan.png`) both succeeded with exactly that set,
and `cargo tauri build --debug` produced a working MSI and NSIS installer with the sidecar and
every resource staged correctly next to the application executable.

**One placement detail this session found and fixed while that wiring was in place:** the
sidecar's DLLs must sit in the same directory as `tesseract.exe` itself once bundled, not in a
nested `resources/` subfolder - Windows only searches the launching executable's own directory
(plus `PATH` and the system directories) for its dependency DLLs, and Tauri copies a sidecar to
the application root rather than into `resources/`. The working config mapped
`resources/tesseract/*` to `"./"` for exactly this reason; `pdfium.dll`, opened directly by our own
process through an explicit path, has no such constraint and stayed under `resources/pdfium/`.
That mapping is recorded here so it does not have to be rediscovered:

```json
"bundle": {
  "externalBin": ["binaries/tesseract"],
  "resources": {
    "resources/tesseract/*": "./",
    "resources/tessdata/fra.traineddata": "resources/tessdata/fra.traineddata",
    "resources/pdfium/pdfium.dll": "resources/pdfium/pdfium.dll"
  }
}
```

**Why it is not in `tauri.conf.json` right now.** `tauri-build`'s build script validates every
`externalBin` and `resources` path on **every** `cargo build`/`cargo test`, not only on
`cargo tauri build`. The files above are fetched by `scripts/fetch-ocr-resources.ps1` into this
gitignored tree, so they exist on this machine - but not on a fresh clone, not on Elise's other
machine, and not on `.github/workflows/ci.yml`'s `windows-latest` or `macos-latest` runners, which
do not run that script. Committing the block above as it stands would break `cargo test --lib`
everywhere except here. Re-add it only together with:

1. A `scripts/fetch-ocr-resources-macos.sh` (Homebrew `tesseract`, `pdfium-binaries`'s macOS
   build), so both CI platforms in section O of `docs/SPRINT-2.5-ASSESSMENT.md` can stage the
   files themselves.
2. A step in `.github/workflows/ci.yml`'s `desktop` job, before `cargo test`, that runs the
   platform-appropriate fetch script.

**Not yet done, beyond that.** The DLL set was pinned by trial and error on one Tesseract build
(`v5.4.0.20240606`) on one machine; it has not been re-verified after a version bump. No macOS
binaries exist yet - `docs/SPRINT-2.5-ASSESSMENT.md` section L's Homebrew-`tesseract` path is the
natural fetch, and its dylib set needs the same trial-removal treatment `install_name_tool -L` (or
`otool -L`) makes possible on macOS, the equivalent of what this session did with
`Get-ChildItem`/deletion on Windows. Prebuilt pdfium for macOS already exists at the same GitHub
release used here (`pdfium-mac-x64.tgz`, `pdfium-mac-arm64.tgz`); only the fetch script for it is
missing.
