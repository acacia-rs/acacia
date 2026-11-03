#!/usr/bin/env bash
# Times the builder's unit-test loop, edit to verdict: touches a builder source (cargo sees an edit), runs
# tools/remote-test.sh in the foreground and splits each run into launch (sync + start), compile and the rest.
# Prints one line a run, then the medians. Research and numbers: research/iteration-speed.md.
# usage: [RUNS=5] [TOUCH=crates/acacia-bot/src/build/plan.rs] [REMOTE_DIR=...] [REMOTE_ENV=...] \
#          tools/iter-bench.sh [cargo test args]   default args: -j 6 -p acacia-bot --lib -- build::
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/$(basename "$root")}
touch_file=$root/${TOUCH:-crates/acacia-bot/src/build/plan.rs}
[ $# -gt 0 ] || set -- -j 6 -p acacia-bot --lib -- build::
rows=()
for ((i = 1; i <= ${RUNS:-5}; i++)); do
  touch "$touch_file"
  t0=$(date +%s.%N)
  DETACH=1 REMOTE_DIR=$remote bash "$root/tools/remote-test.sh" "$@" >/dev/null || exit 1
  t1=$(date +%s.%N)
  out=$(REMOTE_DIR=$remote bash "$root/tools/testbox-wait.sh" "$remote" test)
  code=$?
  t2=$(date +%s.%N)
  compile=$(ssh testbox "grep -oE 'Finished .* in [0-9.]+(s|m [0-9]+s)' $remote/test.log" | sed -E 's/.* in //; s/m /*60+/; s/s$//' | bc)
  row=$(awk -v a="$t0" -v b="$t1" -v c="$t2" -v k="${compile:-0}" -v e="$code" \
    'BEGIN { printf "launch %.1f s  compile %.1f s  rest %.1f s  total %.1f s  exit %d", b - a, k, c - b - k, c - a, e }')
  echo "run $i: $row"
  rows+=("$row")
done
printf '%s\n' "${rows[@]}" | awk '{ l[NR] = $2; k[NR] = $5; r[NR] = $8; t[NR] = $11 }
  function med(a, n,   i, j, x) { for (i = 1; i <= n; i++) for (j = i + 1; j <= n; j++) if (a[j] < a[i]) { x = a[i]; a[i] = a[j]; a[j] = x }
    return n % 2 ? a[(n + 1) / 2] : (a[n / 2] + a[n / 2 + 1]) / 2 }
  END { printf "median of %d: launch %.1f s  compile %.1f s  rest %.1f s  total %.1f s\n", NR, med(l, NR), med(k, NR), med(r, NR), med(t, NR) }'
