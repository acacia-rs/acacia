#!/usr/bin/env bash
# A viewer screenshot taken on testbox (Xvfb, Mesa's lavapipe) against a testbox BDS, copied back here.
# usage: tools/testbox-shot.sh [out.png]   (env: BDS_PORT (19170), REMOTE_DIR, and what viewer-shot.sh and
#        the viewer read: ACACIA_SHOT_AFTER, ACACIA_LOOK, ACACIA_COMMANDS, ACACIA_RISE, ACACIA_DEBUG (F3 shown),
#        ACACIA_SCREEN (inventory open), ACACIA_MENU (pause menu open), ACACIA_PLAY (first person, not the free camera), ACACIA_SETTINGS (a settings file there, e.g. {"look":"java"}), NAME, RADIUS)
# The BDS must be running (tools/testbox-bds.sh start). lavapipe compiles each pipeline on its first
# draw, which stalls one frame 6-8 s after joining: shoot later than that (ACACIA_SHOT_AFTER, default 15). Builds no Windows binary: this machine runs short of memory.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/$(basename "$root")-shot}
out=${1:-$root/target/testbox-shot.png}
vars=()
for v in ACACIA_SHOT_AFTER ACACIA_LOOK ACACIA_COMMANDS ACACIA_COMMANDS_AFTER ACACIA_RISE ACACIA_DEBUG ACACIA_SCREEN ACACIA_MENU ACACIA_PLAY ACACIA_SETTINGS NAME RADIUS LOG RUST_LOG; do
  [ -n "${!v:-}" ] && vars+=("$(printf '%q' "$v=${!v}")")
done
shot="rm -f target/shot.png && cargo +stable build --release -q -p acacia-viewer && env ${vars[*]} xvfb-run -a -s '-screen 0 1280x720x24' bash tools/viewer-shot.sh 127.0.0.1:${BDS_PORT:-19170} target/shot.png"
REMOTE_DIR=$remote TAIL=${TAIL:-20} "$root/tools/remote-run.sh" sh -c "$shot"
mkdir -p "$(dirname "$out")"
scp -q -o ConnectTimeout=20 "testbox:$remote/target/shot.png" "$out" && echo "shot: $out"
