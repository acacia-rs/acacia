#!/usr/bin/env bash
# Interleaved A/B of two sapling bench_server binaries under the 100-bot load, judged by instructions.
# usage: tools/bench100-ab.sh <out dir> <bin A> <bin B> [pairs (2)]   env as tools/bench100.sh; PORT default 19490.
# Prints, per run, bots disconnected and tick-thread instructions per tick in the steady window.
# A run with disconnects is not comparable: drop it.
set -uo pipefail
tools=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: bench100-ab.sh <out dir> <bin A> <bin B> [pairs]} a=${2:?bin A} b=${3:?bin B} pairs=${4:-2}
mkdir -p "$out"
export PORT=${PORT:-19490} PROFILE=1 PROFILE_FREQ=${PROFILE_FREQ:-49}
export IDLE=${IDLE:-10} MIN_SETTLE=${MIN_SETTLE:-120} MAX_SETTLE=${MAX_SETTLE:-150} STEADY=${STEADY:-45}
for ((i = 1; i <= pairs; i++)); do
  for side in a b; do
    bin=$a; [ "$side" = b ] && bin=$b
    run=$out/$side$i
    SAPLING_BIN=$bin bash "$tools/bench100.sh" run sapling "$run" >"$run.log" 2>&1
    bash "$tools/bench100-profile.sh" report "$run" >"$run.report" 2>&1
    python3 -I "$tools/bench100-ab-line.py" "$run" "$run.report"
  done
done
echo done >"$out/run.done"
