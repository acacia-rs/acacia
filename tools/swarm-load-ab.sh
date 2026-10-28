#!/usr/bin/env bash
# Runs on testbox: two swarm_load builds side by side on one tools/swarm-load.sh scenario, each with its
# own server, port and cores (swapped every round), then each run's and each side's mean cycles,
# instructions and swarm CPU. Instructions repeat to ~0.2% between runs whatever the box load, so one
# round settles a few-percent change; `A A` as both sides checks that.
# usage: tools/swarm-load-ab.sh <scenario> <A swarm_load> <B swarm_load> [rounds]   (default 1)
#   PERF=record also profiles each run (default stat: counts only).
set -uo pipefail
scenario=${1:?usage: tools/swarm-load-ab.sh <scenario> <A bin> <B bin> [rounds]}
a=${2:?A bin} b=${3:?B bin} rounds=${4:-1}
out=swarm-load/ab-$scenario
rm -rf "$out"
# LOCK=1: wait for ~/bench.lock, so profiles other sessions take under it keep the box to themselves.
if [ -n "${LOCK:-}" ]; then
  exec 9>"$HOME/bench.lock"
  flock 9
  unset LOCK
fi
(cd "${SAPLING:-../sapling-load}" && RUSTFLAGS="-C force-frame-pointers=yes" cargo +stable build -q --release -p sapling --example load_server 2>/dev/null) || exit 1
# One side's server, port and swarm cores; the other side gets the other set.
slot() {
  case $1 in
    0) echo "19192 4,10 0,1,6,7" ;;
    1) echo "19193 5,11 2,3,8,9" ;;
  esac
}
side() {
  local name=$1 bin=$2 port server swarm
  read -r port server swarm <<<"$(slot "$3")"
  PERF=${PERF:-stat} SWARM_BIN=$bin PORT=$port SERVER_CPUS=$server SWARM_CPUS=$swarm \
    bash tools/swarm-load.sh "$scenario" "$out/$name" >/dev/null || echo "$name failed"
}
for r in $(seq "$rounds"); do
  s=$((r % 2))
  side "A$r" "$a" $s &
  side "B$r" "$b" $((1 - s)) &
  wait
done
row() {
  local c i cpu
  c=$(awk '/ cycles/ {gsub(",", "", $1); print $1}' "$1/stat.txt")
  i=$(awk '/ instructions/ {gsub(",", "", $1); print $1}' "$1/stat.txt")
  cpu=$(grep -o 'swarm CPU: [0-9]*' "$1/summary.txt" | grep -o '[0-9]*$')
  rss=$(grep -o 'swarm memory: VmHWM: [0-9]*' "$1/summary.txt" | grep -o '[0-9]*$')
  echo "$(basename "$1") $c $i $cpu ${rss:-0}"
}
{
  echo "run cycles instructions swarm_cpu_ms peak_rss_mb"
  for d in "$out"/[AB]*; do row "$d"; done
} | tee "$out/runs.txt" | column -t
echo "== means (B/A)"
awk 'NR > 1 { s = substr($1, 1, 1); n[s]++; c[s] += $2; i[s] += $3; m[s] += $4; r[s] += $5 }
  END { for (k in c) printf "%s cycles %.3fG instructions %.3fG cpu %.0f ms rss %.0f MB\n", k, c[k]/n[k]/1e9, i[k]/n[k]/1e9, m[k]/n[k], r[k]/n[k];
        printf "B/A cycles %.3f instructions %.3f cpu %.3f rss %.3f\n", (c["B"]/n["B"])/(c["A"]/n["A"]), (i["B"]/n["B"])/(i["A"]/n["A"]), (m["B"]/n["B"])/(m["A"]/n["A"]), (r["B"]/n["B"])/(r["A"]/n["A"] + 1e-9) }' "$out/runs.txt" | sort | tee "$out/means.txt"
grep -h '^spawned' "$out"/*/summary.txt | sort | uniq -c
