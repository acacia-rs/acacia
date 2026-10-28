#!/usr/bin/env bash
# Interleaved A/B of the light bench (and the one-thread full bench) between two checkouts built by
# profile_light.sh: tools/ab_light.sh <checkout A> <checkout B> [rounds=2]. One line per run.
set -euo pipefail
a=$1 b=$2 rounds=${3:-2}
ev=instructions,cycles,branch-misses,L1-dcache-load-misses,LLC-load-misses
run() { # label checkout args…
  local label=$1 dir=$2
  shift 2
  local out stats
  out=$(mktemp)
  stats=$(ANVIL_THREADS=1 perf stat -x, -e $ev -o /dev/stderr -- "$@" 2>&1 >"$out" | awk -F, '/^[0-9]/ {printf "%s=%.4g ", $3, $1}')
  echo "$label $(basename "$dir") $stats| $(tr '\n' ' ' <"$out")"
  rm -f "$out"
}
for r in $(seq "$rounds"); do
  for dir in "$a" "$b"; do
    bin=$dir/target/profiling/examples
    run area "$dir" "$bin/light" side=32 iters=5 mode=area
    run column "$dir" "$bin/light" side=16 iters=3 mode=column
    run bench "$dir" "$bin/bench" 32 "$dir/target/prof/bench-world"
    run par12 "$dir" "$bin/light" mode=par side=64 iters=2 threads=12
  done
done
