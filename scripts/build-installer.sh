#!/usr/bin/env bash
# Builds the installer with its helper programs inside (decision of 19 October 2026, docs/DECISIONS.md,
# "Where the helper programs and the models live"): the two speech workers and the Tesseract
# sidecar are bundled, the models stay online.
#
# ONE place says which programs go in: `bundle.externalBin` of apps/desktop/src-tauri/tauri.bundle.conf.json.
# This script reads that list, builds and stages what it can (the two workers), checks that every
# other file the list and `bundle.resources` name is on disk, and only then runs `tauri build` with that
# file as an extra configuration. tauri.conf.json is not changed: Tauri checks every `externalBin` file
# at EVERY build, test run included, so declaring them there would stop `cargo test` and `tauri dev`
# on any machine that has not staged them (docs/SPEECH.md, spike S3).
#
# Run once before: scripts/fetch-speech-resources.sh (sherpa-onnx libraries) and, for Tesseract and pdfium,
# scripts/fetch-ocr-resources.ps1. Extra arguments go to `tauri build` (for example `--bundles nsis`).
# Windows entries only: the macOS ones wait for the macOS phase (docs/SESSION-SPEECH, phase 14).
# Usage: bash scripts/build-installer.sh [tauri build arguments]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI="${ROOT}/apps/desktop/src-tauri"
CONFIG="tauri.bundle.conf.json"

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
EXT=""
case "${TRIPLE}" in
  *windows*) EXT=".exe" ;;
  *) echo "the installer is wired for Windows only for now (target: ${TRIPLE})" >&2; exit 1 ;;
esac

cd "${TAURI}"
# What the configuration names, one path per line.
list() {
  node -e '
    const bundle = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).bundle;
    const wanted = process.argv[2] === "programs" ? bundle.externalBin : Object.keys(bundle.resources);
    for (const entry of wanted) console.log(entry);
  ' "${CONFIG}" "$1"
}

while IFS= read -r program; do
  name="${program#binaries/}"
  case "${name}" in
    speech-stt-worker) bash "${ROOT}/scripts/build-speech-worker.sh" ;;
    speech-tts-worker) bash "${ROOT}/scripts/build-speech-tts-worker.sh" ;;
  esac
  if [ ! -f "${TAURI}/${program}-${TRIPLE}${EXT}" ]; then
    echo "missing ${program}-${TRIPLE}${EXT}: stage it first (scripts/fetch-ocr-resources.ps1 stages tesseract)" >&2
    exit 1
  fi
  echo "OK  program ${name}"
done < <(list programs)

while IFS= read -r resource; do
  # shellcheck disable=SC2086
  if ! compgen -G "${TAURI}/${resource}" > /dev/null; then
    echo "missing resource ${resource}: run scripts/fetch-ocr-resources.ps1 or the worker build scripts" >&2
    exit 1
  fi
  echo "OK  resource ${resource}"
done < <(list resources)

cd "${ROOT}/apps/desktop"
pnpm tauri build --config "src-tauri/${CONFIG}" "$@"
echo "Installer under ${TAURI}/target/release/bundle/"
