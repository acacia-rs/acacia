#!/usr/bin/env bash
# One live action run on a fresh testbox BDS: install/reset/start, run examples/actions, stop.
# usage: tools/live-actions.sh [idle]   (env: BDS_DIR/BDS_PORT pick the instance, ONLY=a,b a subset, MOUSE=1 mouse input, RUST_LOG,
#        REMOTE_DIR the synced worktree; prints the PASS/FAIL lines)
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
port=${BDS_PORT:-19170}
log=$(mktemp)
"$root/tools/testbox-bds.sh" stop
"$root/tools/testbox-bds.sh" install >/dev/null && "$root/tools/testbox-bds.sh" reset && "$root/tools/testbox-bds.sh" start | tail -1 || exit 1
args=(run --release -p acacia-bot --example actions -- "127.0.0.1:$port" ActionBot "$@")
# remote-run adds +stable only when the command starts with `cargo`
vars=()
for v in ONLY RUST_LOG MOUSE; do [ -n "${!v:-}" ] && vars+=("$v=${!v}"); done
if [ ${#vars[@]} -gt 0 ]; then cmd=(env "${vars[@]}" cargo +stable "${args[@]}"); else cmd=(cargo "${args[@]}"); fi
REMOTE_DIR=${REMOTE_DIR:-work/$(basename "$root")} TAIL=200 timeout 1700 "$root/tools/remote-run.sh" "${cmd[@]}" >"$log" 2>&1
echo "run exit $?"
"$root/tools/testbox-bds.sh" stop
grep -E '^(PASS|FAIL)|passed, |panicked' "$log"
echo "full log: $log"
