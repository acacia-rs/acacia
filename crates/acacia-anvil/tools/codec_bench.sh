#!/usr/bin/env bash
# Chunk compression options on Linux (README "Compression"): tools/codec_bench.sh [lab corpus dir]
# 1. examples/codec on the lab corpus (a crawled world, if given) and a 128² scale-harness world;
# 2. `perf stat` of examples/bench 32 on one thread per compression (light + encode + compress + write).
set -euo pipefail
cd "$(dirname "$0")/../../.."
cargo build -q --release -p acacia-anvil --example codec --example bench --example scale
ex=target/release/examples
if [ ! -d target/codec-scale/world ]; then
  "$ex/scale" side=128 scale=1000 out=target/codec-scale >/dev/null
fi
for corpus in ${1:+"$1"} target/codec-scale/world; do
  "$ex/codec" "$corpus" --iters "${ITERS:-5}"
done
for c in zlib zlib:3 zlib:1 lz4 none; do
  echo "== bench 32, $c"
  ANVIL_THREADS=1 ANVIL_COMPRESSION=$c perf stat -e instructions,cycles,task-clock -- "$ex/bench" 32 target/codec-bench 2>&1 \
    | grep -E 'columns|instructions|cycles|task-clock'
done
rm -rf target/codec-bench
