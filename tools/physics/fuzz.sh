#!/bin/sh
# usage: [FUZZ_BOTS=n] [ACCOUNT=@name] tools/physics/fuzz.sh <rounds> [seed] [ticks]
# Fuzzes the strict local BDS (tools/bds.ps1 -Strict), records to .testserver/traces, prints the replay summary
# per bot. See docs/testing.md "Movement physics".
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
OUT=$R/.testserver/traces
mkdir -p "$OUT"
if ! cargo build -q -j 1 -p acacia-bot --example fuzz --example replay --manifest-path "$R/Cargo.toml" > "$OUT/build.log" 2>&1; then
  grep -E '^error' -A6 "$OUT/build.log" | head -20; exit 1
fi
T=$OUT/fuzz-$(date +%m%d-%H%M%S).btrc
BOTS=${FUZZ_BOTS:-1}
echo "trace $T"
env -C "$R" NO_COLOR=1 RUST_LOG=${RUST_LOG:-warn} MSYS_NO_PATHCONV=1 BEDROCK_RECORD="$T" FUZZ_BOTS=$BOTS \
  timeout 1800 "$R/target/debug/examples/fuzz.exe" 127.0.0.1:19140 "${ACCOUNT:-@default}" "$@" > "$OUT/fuzz.log" 2>&1
grep -E 'seed|Error|panicked' "$OUT/fuzz.log" | head -5
if [ "$BOTS" -gt 1 ]; then traces=$(ls "$T"-*); else traces=$T; fi
for t in $traces; do echo "trace $t"; "$R/target/debug/examples/replay.exe" "$t" | head -1; done
