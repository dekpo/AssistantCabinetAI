# Builds the speech-to-text worker (apps\speech-worker, binary speech-stt-worker) against the no-tts
# sherpa-onnx libraries, checks that the executable holds no espeak-ng (GPL-3.0) code, and stages it in
# apps\desktop\src-tauri\binaries\ under the name of its target triple, where the application looks for
# it (git-ignored, like the Tesseract sidecar).
#
# Windows twin of build-speech-worker.sh. The text-to-speech worker of a later phase links the full
# libraries and needs its own build with its own CARGO_TARGET_DIR: one build cannot give both. This script
# only builds the speech-to-text one, into apps\speech-worker\target-stt.
#
# Run fetch-speech-resources.ps1 first.

$ErrorActionPreference = 'Stop'
$version = '1.13.8'
$root    = Split-Path -Parent $PSScriptRoot
$libDir  = Join-Path $root "apps\speech-worker\vendor\sherpa-onnx-v$version-win-x64-static-MT-Release-no-tts-lib\lib"
if (-not (Test-Path $libDir)) { throw "missing $libDir : run scripts\fetch-speech-resources.ps1 first" }

$triple = ((rustc -vV) | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
if ($triple -notlike '*windows*') { throw "this script builds for Windows; use build-speech-worker.sh on $triple" }

$env:CARGO_TARGET_DIR = Join-Path $root 'apps\speech-worker\target-stt'
$env:SHERPA_ONNX_LIB_DIR = $libDir
cargo build --release --manifest-path (Join-Path $root 'apps\speech-worker\Cargo.toml') --bin speech-stt-worker
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$built = Join-Path $env:CARGO_TARGET_DIR 'release\speech-stt-worker.exe'
# The check that this executable is the licence-clean one: no match of "espeak", in any case.
$text = [System.Text.Encoding]::GetEncoding(28591).GetString([System.IO.File]::ReadAllBytes($built))
if ($text -match '(?i)espeak') {
    throw "refusing to stage: $built contains 'espeak'; it was linked against the full libraries"
}
"OK  no 'espeak' in speech-stt-worker.exe"

$binaries = Join-Path $root 'apps\desktop\src-tauri\binaries'
New-Item -ItemType Directory -Force $binaries | Out-Null
$staged = Join-Path $binaries "speech-stt-worker-$triple.exe"
Copy-Item $built $staged -Force
"OK  staged $staged"
