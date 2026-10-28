#!/usr/bin/env bash
# Live builder scenes in parallel: one release build of examples/build, then every scene at once, each
# on a fresh world of its own leased pool BDS (tools/bds-pool.sh), then each scene's verdict line.
# Run on testbox through tools/remote-run.sh, or straight from a synced dir there.
# usage: [EXAMPLE=build] [LIVE_ENV="ACACIA_TICK_SCALE=2"] tools/scenes.sh <scene>...
# Logs: scene-<scene>.log, journals: journal-<scene>/.
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
export PATH=$HOME/.cargo/bin:$PATH
example=${EXAMPLE:-build}
(cd "$here" && cargo +stable build --release -q -p acacia-bot --example "$example") || exit 1
bin=$here/target/release/examples/$example
for scene in "$@"; do
  # shellcheck disable=SC2016,SC2086
  bash "$here/tools/bds-pool.sh" run env ${LIVE_ENV:-} SCENE="$scene" BUILD_JOURNAL="$here/journal-$scene" \
    sh -c 'exec "$0" 127.0.0.1:$BDS_PORT "B$BDS_PORT" "O$BDS_PORT"' "$bin" >"$here/scene-$scene.log" 2>&1 &
done
wait
for scene in "$@"; do
  line=$(grep -hE '(PASS|FAIL) after' "$here/scene-$scene.log" | tail -n 1)
  echo "${line:-$scene ERROR: $(tail -n 2 "$here/scene-$scene.log" | tr '\n' ' ')}"
done
