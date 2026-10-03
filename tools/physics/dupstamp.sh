#!/bin/sh
# usage: tools/physics/dupstamp.sh <trace> [correction tolerance, default 0.0002]
# Separates network jitter from physics: counts correction mismatches within 3 ticks after a `minecraft:movement`
# update repeating its stamp (BDS ran a server tick without one of our inputs, so per-server-tick state such as
# the powder snow freeze moved on). Mismatches listed under "others" are worth investigating.
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
REPLAY=$R/target/debug/examples/replay.exe
T=$1; TOL=${2:-0.0002}
TMP=$(mktemp -d)
RUST_LOG=acacia_bot::movement=debug "$REPLAY" "$T" 2>&1 | grep "movement attribute" | sed -E 's/.*server_tick=([0-9]+).*/\1/' | uniq -d > "$TMP/dups"
"$REPLAY" "$T" --correction-tolerance "$TOL" --verbose | grep "^corr" | awk '{print $2}' | grep -v "^1$" > "$TMP/mm"
awk 'NR==FNR {d[$1]=1; nd++; next} {m++; hit=0; for (s=$1-3; s<$1; s++) if (s in d) hit=1; if (hit) h++; else miss=miss " " $1}
  END {printf "dup stamps %d, mismatches %d, after a dup %d, others:%s\n", nd, m, h, miss}' "$TMP/dups" "$TMP/mm"
rm -rf "$TMP"
