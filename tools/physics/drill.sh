#!/bin/sh
# usage: [ACCOUNT=@name] tools/physics/drill.sh <drill,names>
# Runs drills (crates/acacia-bot/examples/drills.rs) on the strict local BDS, records to .testserver/traces, prints
# per-drill corrections and the replay summary.
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
OUT=$R/.testserver/traces
mkdir -p "$OUT"
if ! cargo build -q -j 1 -p acacia-bot --example drills --example replay --manifest-path "$R/Cargo.toml" > "$OUT/build.log" 2>&1; then
  grep -E '^error' -A6 "$OUT/build.log" | head -20; exit 1
fi
T=$OUT/drill-$(date +%m%d-%H%M%S).btrc
echo "trace $T"
env -C "$R" NO_COLOR=1 RUST_LOG=${RUST_LOG:-warn} MSYS_NO_PATHCONV=1 BEDROCK_RECORD="$T" \
  timeout 600 "$R/target/debug/examples/drills.exe" 127.0.0.1:19140 "${ACCOUNT:-@default}" "$1" > "$OUT/drill.log" 2>&1
grep -E 'Error|panicked|corrections' "$OUT/drill.log" | head -40
"$R/target/debug/examples/replay.exe" "$T" | head -1
