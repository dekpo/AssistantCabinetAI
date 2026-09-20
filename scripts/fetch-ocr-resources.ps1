# Fetches the OCR sidecar and its resources for local development on Windows.
#
# None of what this script downloads is committed to git: the Tesseract sidecar, its runtime
# DLLs, the pdfium library and the fra.traineddata model weight are exactly the kind of binary
# blob docs/../.cursor/rules/git-workflow.mdc forbids ("Never commit ... model weights"), the
# same reason Ollama's own weights stay out of git (see .gitignore's `.ollama/`, `models/*.gguf`).
#
# Run this once before `cargo build`/`cargo test` if apps/desktop/src-tauri/tauri.conf.json
# declares `bundle.externalBin`/`bundle.resources` for OCR (see binaries/README.md for why that
# wiring is added in a later commit than this script, not this one).
#
# What it does, in order:
#   1. Downloads the UB-Mannheim Tesseract 5 Windows installer and runs it silently.
#   2. Copies tesseract.exe and only the DLLs it actually needs (verified by trial removal - the
#      installer bundles pango/cairo/icu/fontconfig for its text2image tool, none of which
#      tesseract.exe itself loads) into resources/tesseract/.
#   3. Downloads the pdfium prebuilt Windows x64 library from bblanchon/pdfium-binaries.
#   4. Downloads fra.traineddata (LSTM) from tesseract-ocr/tessdata.
#
# macOS equivalents (Homebrew tesseract + pdfium-binaries macOS build) are not automated here;
# see docs/SPRINT-2.5-ASSESSMENT.md section L. Same resources, same layout, different fetch.

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$root = Resolve-Path "$PSScriptRoot/../apps/desktop/src-tauri"
$binaries = "$root/binaries"
$resources = "$root/resources"
$work = Join-Path $env:TEMP "acai-ocr-fetch"

New-Item -ItemType Directory -Force -Path $binaries, "$resources/tesseract", "$resources/tessdata", "$resources/pdfium", $work | Out-Null

# Tesseract sidecar and its runtime DLLs.
$tesseractRelease = "https://api.github.com/repos/UB-Mannheim/tesseract/releases/latest"
$release = Invoke-WebRequest -Uri $tesseractRelease -UseBasicParsing -Headers @{ "User-Agent" = "assistant-cabinet-ai" } |
    Select-Object -ExpandProperty Content | ConvertFrom-Json
$installerAsset = $release.assets | Where-Object { $_.name -like "tesseract-ocr-w64-setup-*.exe" } | Select-Object -First 1
$installerPath = Join-Path $work $installerAsset.name
Invoke-WebRequest -Uri $installerAsset.browser_download_url -OutFile $installerPath -UseBasicParsing

$installDir = "$env:ProgramFiles\Tesseract-OCR"
if (-not (Test-Path "$installDir\tesseract.exe")) {
    Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait
}

Copy-Item "$installDir\tesseract.exe" "$binaries/tesseract-x86_64-pc-windows-msvc.exe" -Force

# Verified empirically: tesseract.exe loads these and no others. libpango*, libcairo*, libglib*,
# libgio*, libgobject*, libgmodule*, libicu*75.dll, libfontconfig*, libfreetype*, libfribidi*,
# libgraphite2*, libharfbuzz*, libthai*, libdatrie*, libpixman* belong to the installer's
# text2image tool, not to recognition. Removing them and re-running `tesseract --version`
# reproduces this list; see the branch that added this script for the trial-removal session.
$requiredDlls = @(
    "libarchive-13.dll", "libb2-1.dll", "libbrotlicommon.dll", "libbrotlidec.dll", "libbz2-1.dll",
    "libcrypto-3-x64.dll", "libdeflate.dll", "libexpat-1.dll", "libffi-8.dll", "libgcc_s_seh-1.dll",
    "libgif-7.dll", "libiconv-2.dll", "libintl-8.dll", "libjbig-0.dll", "libjpeg-8.dll",
    "libleptonica-6.dll", "libLerc.dll", "liblz4.dll", "liblzma-5.dll", "libopenjp2-7.dll",
    "libpcre2-8-0.dll", "libpng16-16.dll", "libsharpyuv-0.dll", "libstdc++-6.dll",
    "libtesseract-5.dll", "libtiff-6.dll", "libwebp-7.dll", "libwebpmux-3.dll",
    "libwinpthread-1.dll", "libzstd.dll", "zlib1.dll"
)
foreach ($dll in $requiredDlls) {
    Copy-Item "$installDir\$dll" "$resources/tesseract/$dll" -Force
}

# pdfium, for rasterising image-only PDF pages.
$pdfiumRelease = "https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest"
$pdfium = Invoke-WebRequest -Uri $pdfiumRelease -UseBasicParsing -Headers @{ "User-Agent" = "assistant-cabinet-ai" } |
    Select-Object -ExpandProperty Content | ConvertFrom-Json
$pdfiumAsset = $pdfium.assets | Where-Object { $_.name -eq "pdfium-win-x64.tgz" } | Select-Object -First 1
$pdfiumArchive = Join-Path $work $pdfiumAsset.name
Invoke-WebRequest -Uri $pdfiumAsset.browser_download_url -OutFile $pdfiumArchive -UseBasicParsing
$pdfiumExtracted = Join-Path $work "pdfium-extracted"
New-Item -ItemType Directory -Force -Path $pdfiumExtracted | Out-Null
tar -xzf $pdfiumArchive -C $pdfiumExtracted
Copy-Item "$pdfiumExtracted/bin/pdfium.dll" "$resources/pdfium/pdfium.dll" -Force
Copy-Item "$pdfiumExtracted/LICENSE" "$resources/pdfium/LICENSE" -Force

# The French LSTM model. Apache 2.0, per docs/SPRINT-2.5-ASSESSMENT.md section E.
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/tesseract-ocr/tessdata/main/fra.traineddata" `
    -OutFile "$resources/tessdata/fra.traineddata" -UseBasicParsing

Write-Host "OCR resources staged under $resources and $binaries (not tracked by git)."
