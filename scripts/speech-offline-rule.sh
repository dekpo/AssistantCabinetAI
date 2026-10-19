#!/usr/bin/env bash
# Adds, removes or shows the Windows Firewall rule of the offline proof of the speech module
# (docs/SPEECH.md, section 12): it blocks ALL outbound traffic of every program of an INSTALLED copy of
# the application, that is the application and its helper programs (both speech workers and Tesseract),
# and nothing else. The system web view is not blocked (other programs use it); the proof sampler
# reports what it does (scripts/speech-connection-sample.mjs).
#
# It changes the system, so only the owner runs it, from a Git Bash opened "as administrator"
# (`show` needs no administrator rights). One rule per program, all with the same name, removed together.
#
#   bash scripts/speech-offline-rule.sh add    "/c/Users/elise/AppData/Local/Assistant Cabinet AI"
#   bash scripts/speech-offline-rule.sh show
#   bash scripts/speech-offline-rule.sh remove
set -euo pipefail

NAME="AssistantCabinetAI-speech-offline-proof"
ACTION="${1:-}"
export MSYS_NO_PATHCONV=1

case "${ACTION}" in
  add)
    FOLDER="${2:-}"
    if [ -z "${FOLDER}" ] || [ ! -d "${FOLDER}" ]; then
      echo "usage: bash scripts/speech-offline-rule.sh add <install folder>" >&2
      exit 1
    fi
    if ! net session > /dev/null 2>&1; then
      echo "this needs administrator rights: open Git Bash as administrator" >&2
      exit 1
    fi
    COUNT=0
    for PROGRAM in "${FOLDER}"/*.exe; do
      case "$(basename "${PROGRAM}")" in
        uninstall.exe) continue ;;
      esac
      WINDOWS_PATH="$(cygpath -w "${PROGRAM}")"
      netsh advfirewall firewall add rule name="${NAME}" dir=out action=block \
        program="${WINDOWS_PATH}" enable=yes profile=any > /dev/null
      echo "blocked outbound: ${WINDOWS_PATH}"
      COUNT=$((COUNT + 1))
    done
    if [ "${COUNT}" = "0" ]; then
      echo "no program found in ${FOLDER}" >&2
      exit 1
    fi
    echo "Done (${COUNT} programs). Remove the rule when the proof is over: bash scripts/speech-offline-rule.sh remove"
    ;;
  remove)
    if ! net session > /dev/null 2>&1; then
      echo "this needs administrator rights: open Git Bash as administrator" >&2
      exit 1
    fi
    netsh advfirewall firewall delete rule name="${NAME}" || true
    ;;
  show)
    netsh advfirewall firewall show rule name="${NAME}" verbose || true
    ;;
  *)
    echo "usage: bash scripts/speech-offline-rule.sh add <install folder> | remove | show" >&2
    exit 1
    ;;
esac
