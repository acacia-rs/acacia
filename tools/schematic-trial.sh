#!/usr/bin/env bash
# The real-schematic trial (research/schematic-trial.md): one release build of examples/build, then each
# .mcstructure built by one bot (SCHEMATIC=) on a fresh world of a leased pool BDS (tools/bds-pool.sh), at most
# PAR (default 3) at once, then each run's verdict line. Run on testbox through tools/remote-run.sh.
# usage: [PAR=3] [LIVE_ENV="..."] tools/schematic-trial.sh <file.mcstructure>...
# Logs: trial-<name>.log, journals: journal-<name>/.
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
export PATH=$HOME/.cargo/bin:$PATH
(cd "$here" && cargo +stable build --release -q -j 6 -p acacia-bot --example build) || exit 1
bin=$here/target/release/examples/build
run() {
  local name; name=$(basename "$1" .mcstructure)
  # shellcheck disable=SC2016,SC2086
  bash "$here/tools/bds-pool.sh" run env ${LIVE_ENV:-} SCHEMATIC="$1" MAX_TICKS=200000 SHOW_WRONG=400 BUILD_JOURNAL="$here/journal-$name" \
    sh -c 'exec "$0" 127.0.0.1:$BDS_PORT "B$BDS_PORT" "O$BDS_PORT"' "$bin" >"$here/trial-$name.log" 2>&1
  # No verdict line means the run itself broke (an observer login refused AlreadyConnected): one retry.
  [ -n "${2:-}" ] || grep -qE '(PASS|FAIL) after' "$here/trial-$name.log" || { cp "$here/trial-$name.log" "$here/trial-$name.try1.log"; run "$1" retry; }
}
for f in "$@"; do
  while [ "$(jobs -rp | wc -l)" -ge "${PAR:-3}" ]; do wait -n; done
  run "$f" &
done
wait
for f in "$@"; do
  name=$(basename "$f" .mcstructure)
  line=$(grep -hE '(PASS|FAIL) after' "$here/trial-$name.log" | tail -n 1)
  echo "${line:-$name ERROR: $(tail -n 2 "$here/trial-$name.log" | tr '\n' ' ')}"
done
