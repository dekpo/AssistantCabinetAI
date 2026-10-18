#!/usr/bin/env bash
# Fetches the prebuilt sherpa-onnx static libraries the speech workers link against, into
# apps/speech-worker/vendor/ (git-ignored: a large reproducible third-party binary, the same rule as
# the OCR resources of scripts/fetch-ocr-resources.ps1). Run it once before
# scripts/build-speech-worker.sh. It downloads no speech model: the application does that itself,
# on an explicit click in the settings.
#
# Two variants, because the two workers cannot share a build (docs/SPEECH.md):
#   no-tts  for the speech-to-text worker. It has no espeak-ng (GPL-3.0) code in it.
#   full    for the text-to-speech worker of a later phase. It carries espeak-ng.
#
# Why curl and not the crate's own download: the sherpa-onnx-sys build script downloads with a TLS
# stack that does not read the system certificate store, which fails behind an antivirus that
# intercepts TLS (SpeechLab, issue I-009). curl uses the system store. The SHA-256 of each archive
# is pinned below, taken from the digest GitHub publishes for release v1.13.8; a mismatch deletes
# the file and stops.
#
# Windows (Git Bash) and macOS. The macOS archives and digests are pinned but not yet run on a Mac.
# Usage: bash scripts/fetch-speech-resources.sh
set -euo pipefail

VERSION="1.13.8" # must match the sherpa-onnx crate version in apps/speech-worker/Cargo.toml
BASE="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${VERSION}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR="${ROOT}/apps/speech-worker/vendor"

case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    NO_TTS_NAME="sherpa-onnx-v${VERSION}-win-x64-static-MT-Release-no-tts-lib"
    NO_TTS_SHA="f0aa074ddc39553b30208b53e84c12275d09702911c9faa8d34fa932428e4641"
    FULL_NAME="sherpa-onnx-v${VERSION}-win-x64-static-MT-Release-lib"
    FULL_SHA="56ffcf3c454c1f14f7bc9887286cc8143e7e542dc632804e1c447d5f8d534eaf"
    PLACEHOLDERS="espeak-ng.lib piper_phonemize.lib ucd.lib"
    ;;
  Darwin)
    if [ "$(uname -m)" = "arm64" ]; then
      NO_TTS_NAME="sherpa-onnx-v${VERSION}-osx-arm64-static-no-tts-lib"
      NO_TTS_SHA="3d7f9b8a496694af13d9802c33b8133231e397bdef302f543d19468765e83136"
      FULL_NAME="sherpa-onnx-v${VERSION}-osx-arm64-static-lib"
      FULL_SHA="9091bf160dc7fdacedbc906b212badf53c2993f4e5277a0e03998e96c31d60da"
    else
      NO_TTS_NAME="sherpa-onnx-v${VERSION}-osx-x64-static-no-tts-lib"
      NO_TTS_SHA="8ffc3ede9f997fec547b5c99b8e2073b8e04fd48a5f65e1a6d5d314ad0106ad9"
      FULL_NAME="sherpa-onnx-v${VERSION}-osx-x64-static-lib"
      FULL_SHA="a3f88da3e54c850a12d61431e73f8affcd1f13738b75b768847dd79541835b4b"
    fi
    PLACEHOLDERS="libespeak-ng.a libpiper_phonemize.a libucd.a"
    ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

digest() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

fetch() { # name sha
  local name="$1" sha="$2" file="${VENDOR}/${1}.tar.bz2"
  if [ ! -f "${file}" ] || [ "$(digest "${file}")" != "${sha}" ]; then
    curl -L --fail -sS -o "${file}" "${BASE}/${name}.tar.bz2"
  fi
  local actual
  actual="$(digest "${file}")"
  if [ "${actual}" != "${sha}" ]; then
    rm -f "${file}"
    echo "checksum mismatch for ${name}: expected ${sha}, got ${actual}" >&2
    exit 1
  fi
  echo "OK  ${name}  sha256=${actual}"
  if [ ! -d "${VENDOR}/${name}/lib" ]; then
    tar -xjf "${file}" -C "${VENDOR}"
  fi
}

mkdir -p "${VENDOR}"
fetch "${NO_TTS_NAME}" "${NO_TTS_SHA}"
fetch "${FULL_NAME}" "${FULL_SHA}"

# The crate's link list always names espeak-ng, piper_phonemize and ucd, which the no-tts archive
# does not contain. An empty archive (the 8-byte signature and nothing else) satisfies the linker and
# links no code: this is a workaround, not a supported configuration of the crate (SpeechLab D-012).
for placeholder in ${PLACEHOLDERS}; do
  printf '!<arch>\n' > "${VENDOR}/${NO_TTS_NAME}/lib/${placeholder}"
done
echo "OK  placeholders in ${NO_TTS_NAME}/lib: ${PLACEHOLDERS}"
