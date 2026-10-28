#!/usr/bin/env bash
# testbox side of tools/build-ab.sh (runs as its detached job, so a local reap can't stop it): builds swarm_build,
# deals the runs out with variants interleaved, then prints the per-variant summary.
# Without PORTS each run leases a pool BDS (tools/bds-pool.sh) on a fresh world, all at once up to the pool size;
# with PORTS="a b c", one worker per fixed port runs its share one after another.
# Results: build-ab/results; each run's log and journals: build-ab/<variant>-<run>/.
# usage: tools/build-ab-run.sh <runs> <variant>...
set -uo pipefail
runs=$1
shift
cargo +stable build --release -p acacia-bot --example swarm_build 2>&1 | tail -n 3
[ "${PIPESTATUS[0]}" -eq 0 ] || exit 1
bin=$PWD/target/release/examples/swarm_build
out=$PWD/build-ab
rm -rf "$out"
mkdir -p "$out"
read -ra ports <<<"${PORTS:-}"
k=0
for ((r = 1; r <= runs; r++)); do
  for v in "$@"; do
    case $v in *:*) e=${v#*:} ;; *) e= ;; esac
    if [ ${#ports[@]} -gt 0 ]; then
      echo "${v%%:*} $r $e" >>"$out/jobs.${ports[$(( k % ${#ports[@]} ))]}"
    else
      echo "${v%%:*} $r $e" >"$out/job.$k"
      # shellcheck disable=SC2016
      bash tools/bds-pool.sh run sh -c 'exec bash tools/build-ab-worker.sh "$0" "$BDS_PORT" "$1" "$2"' "$bin" "$out/job.$k" "$out" &
    fi
    k=$(( k + 1 ))
  done
done
for p in "${ports[@]}"; do
  [ -f "$out/jobs.$p" ] && bash tools/build-ab-worker.sh "$bin" "$p" "$out/jobs.$p" "$out" &
done
wait
echo "== results"
cat "$out/results"
echo "== summary"
awk '{ n[$1]++; if ($4 == "PASS") p[$1]++; if ($5 != "-") { s[$1] += $5; q[$1] += $5 * $5; c[$1]++ } }
  END { for (v in n) { m = c[v] ? s[v] / c[v] : 0; sd = c[v] > 1 ? sqrt((q[v] - c[v] * m * m) / (c[v] - 1)) : 0;
    printf "%-12s %d/%d pass  mean %.1f s  sd %.1f s\n", v, p[v], n[v], m, sd } }' "$out/results"
