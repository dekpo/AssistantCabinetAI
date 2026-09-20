# Bundled OCR resources

Not committed, for the same reason as `../binaries/README.md`: `fra.traineddata` is a model
weight, and the Tesseract DLLs and the pdfium library are the third-party binaries that read one.
Run `scripts/fetch-ocr-resources.ps1` from the repository root; it stages all three trees below.

```text
resources/tesseract/       tesseract.exe's runtime DLLs (thirty files, verified minimal - see
                            ../binaries/README.md); bundled to the application root, not here,
                            because tesseract.exe needs them beside itself, not beside the app
resources/tessdata/fra.traineddata   the LSTM French model Tesseract loads at recognition time
resources/pdfium/pdfium.dll          the platform's prebuilt pdfium dynamic library, loaded
                                      directly by our own process through an explicit path
```

`Rasterizer::new` (`src/raster.rs`) and `TesseractProvider::new` (`src/ocr/tesseract.rs`) resolve
their paths through `app.path().resolve(...)` at runtime, never a literal, and report
`OcrError::EngineUnavailable` rather than panicking when a resource is missing - the product still
works as Sprint 2a without them.

**Verified in this session, with `bundle.externalBin`/`bundle.resources` temporarily in place:**
`cargo tauri build --debug` produced a working MSI and NSIS installer with
`resources/tessdata/fra.traineddata` and `resources/pdfium/pdfium.dll` staged at those exact paths
under the built application, and the Tesseract DLLs staged at the application root beside the
`tesseract.exe` sidecar. That config is not currently in `tauri.conf.json` - see
`../binaries/README.md` for the exact block to restore and why it waits on a macOS fetch script
and a CI step, not on anything wrong with the resources themselves.

See `docs/SPRINT-2.5-ASSESSMENT.md` sections E and L for where these files come from and their
licences (`fra.traineddata`: Apache 2.0; pdfium: BSD-3-Clause / Apache-2.0; Tesseract: Apache 2.0).
`macOS` equivalents are not yet scripted - see `../binaries/README.md`.
