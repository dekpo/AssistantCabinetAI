# Fetches the prebuilt sherpa-onnx static libraries the speech workers link against, into
# apps\speech-worker\vendor\ (git-ignored: a large reproducible third-party binary, the same rule as the
# OCR resources of fetch-ocr-resources.ps1). Run it once before build-speech-worker.ps1. It downloads no
# speech model: the application does that itself, on an explicit click in the settings.
#
# Windows twin of fetch-speech-resources.sh (the one that also covers macOS). Two variants, because the
# two workers cannot share a build:
#   no-tts  for the speech-to-text worker. It has no espeak-ng (GPL-3.0) code in it.
#   full    for the text-to-speech worker of a later phase. It carries espeak-ng.
#
# Why curl.exe and not the crate's own download: the sherpa-onnx-sys build script downloads with a TLS
# stack that does not read the Windows certificate store, which fails behind an antivirus that
# intercepts TLS (SpeechLab, issue I-009). curl.exe uses the Windows store. The SHA-256 of each archive
# is pinned below, taken from the digest GitHub publishes for release v1.13.8; a mismatch deletes the
# file and stops.

$ErrorActionPreference = 'Stop'
$version = '1.13.8'   # must match the sherpa-onnx crate version in apps\speech-worker\Cargo.toml
$base    = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version"
$root    = Split-Path -Parent $PSScriptRoot
$vendor  = Join-Path $root 'apps\speech-worker\vendor'

$archives = @(
    @{ Name = "sherpa-onnx-v$version-win-x64-static-MT-Release-no-tts-lib"; Sha = 'f0aa074ddc39553b30208b53e84c12275d09702911c9faa8d34fa932428e4641' },
    @{ Name = "sherpa-onnx-v$version-win-x64-static-MT-Release-lib";        Sha = '56ffcf3c454c1f14f7bc9887286cc8143e7e542dc632804e1c447d5f8d534eaf' }
)

New-Item -ItemType Directory -Force $vendor | Out-Null

foreach ($archive in $archives) {
    $file = Join-Path $vendor "$($archive.Name).tar.bz2"
    if (-not (Test-Path $file) -or (Get-FileHash $file -Algorithm SHA256).Hash.ToLower() -ne $archive.Sha) {
        curl.exe -L --fail -sS -o $file "$base/$($archive.Name).tar.bz2"
        if ($LASTEXITCODE -ne 0) { throw "download failed: $($archive.Name)" }
    }
    $actual = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $archive.Sha) {
        Remove-Item $file
        throw "checksum mismatch for $($archive.Name): expected $($archive.Sha), got $actual"
    }
    "OK  $($archive.Name)  sha256=$actual"
    if (-not (Test-Path (Join-Path $vendor "$($archive.Name)\lib"))) {
        tar -xjf $file -C $vendor
        if ($LASTEXITCODE -ne 0) { throw "unpacking failed: $($archive.Name)" }
    }
}

# The crate's link list always names espeak-ng, piper_phonemize and ucd, which the no-tts archive does
# not contain. An empty archive (the 8-byte signature and nothing else) satisfies the linker and links
# no code: a workaround, not a supported configuration of the crate (SpeechLab D-012).
$noTtsLib = Join-Path $vendor "$($archives[0].Name)\lib"
$signature = [System.Text.Encoding]::ASCII.GetBytes("!<arch>`n")
foreach ($placeholder in 'espeak-ng.lib', 'piper_phonemize.lib', 'ucd.lib') {
    [System.IO.File]::WriteAllBytes((Join-Path $noTtsLib $placeholder), $signature)
}
"OK  placeholders in $noTtsLib"
