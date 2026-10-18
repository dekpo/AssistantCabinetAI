#!/usr/bin/env bash
# Builds the speech-to-text worker (apps/speech-worker, binary speech-stt-worker) against the no-tts
# sherpa-onnx libraries, checks that the executable holds no espeak-ng (GPL-3.0) code, and stages it
# in apps/desktop/src-tauri/binaries/ under the name of its target triple, where the application
# looks for it (git-ignored, like the Tesseract sidecar).
#
# The text-to-speech worker of a later phase links the full libraries and needs its own build with
# its own CARGO_TARGET_DIR: one build cannot give both. This script only builds the speech-to-text one,
# into apps/speech-worker/target-stt.
#
# Run scripts/fetch-speech-resources.sh first. Usage: bash scripts/build-speech-worker.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR="${ROOT}/apps/speech-worker/vendor"
VERSION="1.13.8"

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
EXT=""
case "${TRIPLE}" in
  *windows*) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-win-x64-static-MT-Release-no-tts-lib/lib"; EXT=".exe" ;;
  aarch64-apple-darwin) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-osx-arm64-static-no-tts-lib/lib" ;;
  x86_64-apple-darwin) LIB_DIR="${VENDOR}/sherpa-onnx-v${VERSION}-osx-x64-static-no-tts-lib/lib" ;;
  *) echo "unsupported target: ${TRIPLE}" >&2; exit 1 ;;
esac
if [ ! -d "${LIB_DIR}" ]; then
  echo "missing ${LIB_DIR}: run scripts/fetch-speech-resources.sh first" >&2
  exit 1
fi

export CARGO_TARGET_DIR="${ROOT}/apps/speech-worker/target-stt"
export SHERPA_ONNX_LIB_DIR="${LIB_DIR}"
cargo build --release --manifest-path "${ROOT}/apps/speech-worker/Cargo.toml" --bin speech-stt-worker

BUILT="${CARGO_TARGET_DIR}/release/speech-stt-worker${EXT}"
# The check that this executable is the licence-clean one: no match of "espeak", in any case.
MATCHES="$(grep -c -a -i 'espeak' "${BUILT}" || true)"
if [ "${MATCHES}" != "0" ]; then
  echo "refusing to stage: ${BUILT} contains 'espeak' (${MATCHES} matching lines); it was linked against the full libraries" >&2
  exit 1
fi
echo "OK  no 'espeak' in $(basename "${BUILT}")"

STAGED="${ROOT}/apps/desktop/src-tauri/binaries/speech-stt-worker-${TRIPLE}${EXT}"
mkdir -p "$(dirname "${STAGED}")"
cp "${BUILT}" "${STAGED}"
echo "OK  staged ${STAGED}"
