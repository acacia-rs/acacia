#!/usr/bin/env bash
# Linux light profiling (README "Throughput"): tools/profile_light.sh <stat|record|heaptrack|time> <light|bench> [args…]
# light = examples/light.rs (default side=32 iters=5), bench = examples/bench.rs (default 32, one thread).
set -euo pipefail
cd "$(dirname "$0")/../../.."
what=$1 ex=$2
shift 2
export CARGO_PROFILE_PROFILING_DEBUG=line-tables-only
cargo build -q --profile profiling -p acacia-anvil --example "$ex"
bin=target/profiling/examples/$ex
out=target/prof
mkdir -p "$out"
if [ $# -eq 0 ]; then
  if [ "$ex" = light ]; then set -- side=32 iters=5; else set -- 32 "$out/bench-world"; fi
fi
export ANVIL_THREADS=${ANVIL_THREADS:-1}
case $what in
  stat) perf stat -e instructions,cycles,branches,branch-misses,L1-dcache-loads,L1-dcache-load-misses,LLC-loads,LLC-load-misses \
          -- "$bin" "$@" ;;
  record) perf record -q -F 2000 --call-graph dwarf,16384 -o "$out/$ex.data" -- "$bin" "$@" >/dev/null
          perf report -i "$out/$ex.data" --no-children --stdio --percent-limit 1.5 -g none 2>/dev/null | grep -v '^$' | head -40 ;;
  heaptrack) heaptrack -o "$out/$ex" "$bin" "$@" >/dev/null
             heaptrack_print "$out/$ex".zst 2>/dev/null | grep -E '^(total runtime|calls to allocation|temporary|peak heap|peak RSS)' ;;
  time) /usr/bin/time -f 'peak RSS %M KB, %e s wall, %U s user' "$bin" "$@" ;;
  *) echo "unknown: $what" >&2; exit 2 ;;
esac
