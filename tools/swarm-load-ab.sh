#!/usr/bin/env bash
# Runs on testbox: two swarm_load builds in turn (A B A B ...) on one tools/swarm-load.sh scenario, so
# both see the same box load, then each run's and each side's mean cycles, instructions and swarm CPU.
# usage: tools/swarm-load-ab.sh <scenario> <A swarm_load> <B swarm_load> [rounds]   (default 3)
set -uo pipefail
scenario=${1:?usage: tools/swarm-load-ab.sh <scenario> <A bin> <B bin> [rounds]}
a=${2:?A bin} b=${3:?B bin} rounds=${4:-3}
out=swarm-load/ab-$scenario
rm -rf "$out"
for r in $(seq "$rounds"); do
  for side in A B; do
    bin=$a; [ $side = B ] && bin=$b
    SWARM_BIN=$bin bash tools/swarm-load.sh "$scenario" "$out/$side$r" >/dev/null || echo "$side$r failed"
  done
done
row() {
  local c i cpu
  c=$(awk '/ cycles/ {gsub(",", "", $1); print $1}' "$1/stat.txt")
  i=$(awk '/ instructions/ {gsub(",", "", $1); print $1}' "$1/stat.txt")
  cpu=$(grep -o 'swarm CPU: [0-9]*' "$1/summary.txt" | grep -o '[0-9]*$')
  echo "$(basename "$1") $c $i $cpu"
}
{
  echo "run cycles instructions swarm_cpu_ms"
  for d in "$out"/[AB]*; do row "$d"; done
} | tee "$out/runs.txt" | column -t
echo "== means (B/A)"
awk 'NR > 1 { s = substr($1, 1, 1); n[s]++; c[s] += $2; i[s] += $3; m[s] += $4 }
  END { for (k in c) printf "%s cycles %.3fG instructions %.3fG cpu %.0f ms\n", k, c[k]/n[k]/1e9, i[k]/n[k]/1e9, m[k]/n[k];
        printf "B/A cycles %.3f instructions %.3f cpu %.3f\n", (c["B"]/n["B"])/(c["A"]/n["A"]), (i["B"]/n["B"])/(i["A"]/n["A"]), (m["B"]/n["B"])/(m["A"]/n["A"]) }' "$out/runs.txt" | sort | tee "$out/means.txt"
