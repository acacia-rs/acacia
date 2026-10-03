#!/usr/bin/env bash
# Joins a server with acacia-viewer, saves one frame and exits; prints the log tail and the PNG path.
# usage: tools/viewer-shot.sh [server] [out.png]   (env: ACACIA_SHOT_AFTER, ACACIA_LOOK=yaw,pitch, ACACIA_RISE,
#        RADIUS, NAME, BIN, LOG; same flags as the viewer's Shot)
# The viewer exits non-zero when kicked before the shot; a ServerIdConflict kick (BDS still holds the last
# session under this name) is retried a few times.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
server=${1:-127.0.0.1:19140}
out=${2:-$root/target/viewer-shot.png}
bin=${BIN:-$root/target/release/acacia-viewer}
log=${LOG:-$(mktemp)}
for attempt in 1 2 3 4; do
  rm -f "$out"
  ACACIA_SCREENSHOT="$out" ACACIA_ASSETS="${ACACIA_ASSETS:-$root/assets/vanilla}" RUST_LOG=${RUST_LOG:-info,wgpu_core=warn,wgpu_hal=warn} \
    timeout 120 "$bin" "$server" "${NAME:-ViewerShot}" "${RADIUS:-6}" >"$log" 2>&1
  status=$?
  grep -q ServerIdConflict "$log" || break
  echo "attempt $attempt: kicked with ServerIdConflict, retrying in 5 s"
  sleep 5
done
echo "exit $status"
grep -vE "^\s*$" "$log" | tail -n 15
[ -f "$out" ] && echo "shot: $out"
exit $status
