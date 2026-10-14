#!/usr/bin/env bash
# The long retrieval-lab runs, one after the other, each logged with its start and end time. A run that fails does not
# stop the next one. Fictional fixtures only; the gateway is the owner's own tower.
#
#   bash scripts/retrieval_lab_overnight.sh            # from the repository root, Git Bash
#
# Needs: the gateway reachable (it waits up to 30 minutes for it before each run), the big set generated:
#   python scripts/gen-retrieval-lab.py --books-dir "C:/Users/elise/Downloads" --documents 500 --out data/retrieval-lab-big
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATEWAY="${ACAI_GATEWAY:-http://192.168.1.83:8080}"
LOG="$ROOT/data/retrieval-lab/overnight.log"
mkdir -p "$ROOT/data/retrieval-lab"

wait_for_gateway() {
  for _ in $(seq 1 60); do
    if curl -s -m 8 -o /dev/null -w '%{http_code}' "$GATEWAY/health" | grep -q 200; then return 0; fi
    sleep 30
  done
  return 1
}

run() { # label, alias, sandbox dir ("" for the default exam), tier
  local label="$1" alias="$2" sandbox="$3" tier="$4"
  echo "$(date '+%F %T') START $label (alias $alias, tier $tier)" >> "$LOG"
  if ! wait_for_gateway; then echo "$(date '+%F %T') SKIP  $label: the gateway does not answer" >> "$LOG"; return; fi
  (
    cd "$ROOT/apps/desktop/src-tauri" || exit 1
    export ACAI_GATEWAY="$GATEWAY" ACAI_EMBED_ALIAS="$alias" ACAI_TIER="$tier" ACAI_LAB_LABEL="$label"
    if [ -n "$sandbox" ]; then export ACAI_SANDBOX_DIR="$ROOT/$sandbox"; else unset ACAI_SANDBOX_DIR; fi
    cargo test --release --test retrieval_lab -- --ignored --nocapture > "$ROOT/data/retrieval-lab/$label.log" 2>&1
  )
  echo "$(date '+%F %T') END   $label (exit $?)" >> "$LOG"
}

run big-nomic      assistant-embed         data/retrieval-lab-big 1000
run big-bge-m3     bge-m3                  data/retrieval-lab-big 1000
run big-qwen3-0.6b qwen3-embedding:0.6b    data/retrieval-lab-big 1000
run exam-bge-m3    bge-m3                  ""                     200
run exam-qwen3-0.6b qwen3-embedding:0.6b   ""                     200
echo "$(date '+%F %T') ALL DONE" >> "$LOG"
