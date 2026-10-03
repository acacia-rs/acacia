#!/usr/bin/env bash
# Runs on testbox from a synced, built worktree: <runs> live action runs against one BDS instance, each on a
# fresh world. Started by tools/live-repeat.sh.
# usage: tools/live-loop.sh <bds dir> <port> <runs> <out dir> [idle]   (env: ONLY, RUST_LOG, LOG_GREP)
# Appends "<run> PASS|FAIL ..." and pack lines matching $LOG_GREP to <out dir>/results; failing runs keep
# <out dir>/<run>.bot.log and <run>.bds.log. Touches <out dir>/done at the end.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
bds=$(cd "$1" && pwd) || exit 1
port=$2 runs=$3 out=$4
shift 4
mkdir -p "$out" && rm -f "$out"/*
: >"$out/results"
ctl() { (cd "$bds" && bash "$root/tools/bds-ctl.sh" "$1" >/dev/null); }

# Box convention: heavy or timing-sensitive runs hold ~/bench.lock. Shared, so the two instances run
# together; another project's exclusive benchmark makes them wait (BDS timed out under its load).
exec 9>"$HOME/bench.lock"
if ! flock -n -s 9; then
  echo "0 WAIT bench.lock is held by another benchmark; waiting" >>"$out/results"
  flock -s 9
fi

for i in $(seq "$runs"); do
  ctl stop; ctl reset; ctl start
  timeout 1500 "$root/target/release/examples/actions" "127.0.0.1:$port" ActionBot "$@" >"$out/bot.log" 2>&1
  ctl stop
  grep -E '^(PASS|FAIL)' "$out/bot.log" | sed "s/^/$i /" >>"$out/results"
  grep -qE '^(PASS|FAIL)' "$out/bot.log" || echo "$i ERROR $(tail -n 1 "$out/bot.log")" >>"$out/results"
  [ -n "${LOG_GREP:-}" ] && grep -oE "ACTIONTEST ($LOG_GREP).*" "$bds/bds.log" | sed "s/^/$i /" >>"$out/results"
  if grep -qE '^FAIL' "$out/bot.log" || ! grep -qE '^PASS' "$out/bot.log"; then
    cp "$out/bot.log" "$out/$i.bot.log"
    cp "$bds/bds.log" "$out/$i.bds.log"
  fi
done
touch "$out/done"
