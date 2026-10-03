#!/usr/bin/env bash
# Joins a server with acacia-viewer, saves one frame and exits; prints the log tail and the PNG path.
# usage: tools/viewer-shot.sh [server] [out.png]   (env: ACACIA_SHOT_AFTER, ACACIA_LOOK=yaw,pitch, ACACIA_RISE,
#        RADIUS, NAME; same flags as the viewer's Shot)
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
server=${1:-127.0.0.1:19140}
out=${2:-$root/target/viewer-shot.png}
log=$(mktemp)
rm -f "$out"
ACACIA_SCREENSHOT="$out" ACACIA_ASSETS="$root/assets/vanilla" RUST_LOG=${RUST_LOG:-info,wgpu_core=warn,wgpu_hal=warn} \
  timeout 120 "$root/target/release/acacia-viewer" "$server" "${NAME:-ViewerShot}" "${RADIUS:-6}" >"$log" 2>&1
echo "exit $?"
grep -vE "^\s*$" "$log" | tail -n 15
[ -f "$out" ] && echo "shot: $out"
