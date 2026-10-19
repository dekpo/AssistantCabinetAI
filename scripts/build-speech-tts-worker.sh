#!/usr/bin/env bash
# Builds the text-to-speech worker (apps/speech-worker, binary speech-tts-worker) against the FULL
# sherpa-onnx libraries, which carry the GPL-3.0 espeak-ng phonemizer, checks that the phonemizer is
# really in it (the opposite of the speech-to-text check), and stages the executable and its notice in
# apps/desktop/src-tauri/binaries/, under the name of its target triple, where the application looks
# for it (git-ignored, like the speech-to-text worker).
#
# It cannot share a build with the speech-to-text worker: its own CARGO_TARGET_DIR (target-tts), its own
# feature (`tts`, without the default `stt`), its own libraries. The notice file is staged next to the
# executable and must travel with it (apps/speech-worker/NOTICE-TTS-WORKER.md).
#
# Run scripts/fetch-speech-resources.sh first. Usage: bash scripts/build-speech-tts-worker.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR="${ROOT}/apps/speech-worker/vendor"
VERSION="1.13.8"

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
EXT=""
case "${TRIPLE}" in
  # The C runtime is linked statically (the sherpa-onnx libraries are "static-MT"): the executable then needs
  # no Visual C++ Redistributable on a clean Windows (SpeechLab I-070, answered by phase 13).
  *windows*) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-win-x64-static-MT-Release-lib/lib"; EXT=".exe"; export RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+crt-static" ;;
  aarch64-apple-darwin) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-osx-arm64-static-lib/lib" ;;
  x86_64-apple-darwin) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-osx-x64-static-lib/lib" ;;
  *) echo "unsupported target: ${TRIPLE}" >&2; exit 1 ;;
esac
if [ ! -d "${LIB_DIR}" ]; then
  echo "missing ${LIB_DIR}: run scripts/fetch-speech-resources.sh first" >&2
  exit 1
fi

export CARGO_TARGET_DIR="${ROOT}/apps/speech-worker/target-tts"
export SHERPA_ONNX_LIB_DIR="${LIB_DIR}"
cargo build --release --manifest-path "${ROOT}/apps/speech-worker/Cargo.toml" \
  --no-default-features --features tts --bin speech-tts-worker

BUILT="${CARGO_TARGET_DIR}/release/speech-tts-worker${EXT}"
# The check that this executable is the one with the phonemizer in it, so that it ships with the notice
# that says so: at least one match of "espeak", in any case.
MATCHES="$(grep -c -a -i 'espeak' "${BUILT}" || true)"
if [ "${MATCHES}" = "0" ]; then
  echo "refusing to stage: ${BUILT} holds no 'espeak': it was not linked against the full libraries" >&2
  exit 1
fi
echo "OK  'espeak' found in $(basename "${BUILT}") (${MATCHES} matching lines): it ships with its notice"

BINARIES="${ROOT}/apps/desktop/src-tauri/binaries"
mkdir -p "${BINARIES}"
cp "${BUILT}" "${BINARIES}/speech-tts-worker-${TRIPLE}${EXT}"
cp "${ROOT}/apps/speech-worker/NOTICE-TTS-WORKER.md" "${BINARIES}/speech-tts-worker-NOTICE.md"
echo "OK  staged ${BINARIES}/speech-tts-worker-${TRIPLE}${EXT} and its notice"
