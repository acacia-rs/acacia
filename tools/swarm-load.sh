#!/usr/bin/env bash
# Runs on testbox from a synced worktree: a swarm against sapling's load_server (a busy world: a crowd
# that sees itself, redstone clocks, strolling pigs) with perf over the swarm once all have joined.
# Send it with tools/remote-run.sh; tools/swarm-load-suite.sh runs every scenario and keeps the reports.
# usage: tools/swarm-load.sh <scenario> [out dir] | tools/swarm-load.sh report <out dir>
#   scenarios: crowd wander redstone mobs mixed mixed150 (table below: bots, wanderers, clocks, mobs)
#   SAPLING=<dir> is a sapling checkout with the load_server example (default ../sapling-load).
#   SWARM_BIN=<path> runs that swarm_load build instead of building this tree's (tools/swarm-load-ab.sh).
#   Writes <out dir>/report.txt: joins, swarm and server CPU, perf stat over the window, then self time
#   by module and function, then inclusive. The server has its own cores: its CPU near one core means
#   it, not the swarm, set the load. Compare runs by cycles: the box is shared and often busy.
set -uo pipefail
report() { sudo perf report -i "$1/perf.data" "${@:2}" 2>/dev/null | grep -v '^#' | grep -v '^$' | cut -c1-160 || true; }
write_report() {
  {
    cat "$1/summary.txt"
    grep -E 'task-clock|cycles|instructions|context-switches' "$1/stat.txt" | sed 's/  */ /g'
    echo "== self by module"
    report "$1" --no-children --sort dso -g none | head -12
    echo "== self by function"
    report "$1" --no-children --sort symbol -g none | head -50
    echo "== inclusive"
    report "$1" --children --sort symbol -g none | head -80
  } >"$1/report.txt"
}
if [ "${1:-}" = report ]; then
  write_report "${2:?usage: tools/swarm-load.sh report <out dir>}"
  exit
fi

scenario=${1:?usage: tools/swarm-load.sh <scenario> [out dir]}
out=${2:-swarm-load/$scenario}
sapling=${SAPLING:-../sapling-load}
case $scenario in
  crowd) set -- 50 0 0 0 ;;
  wander) set -- 50 50 0 0 ;;
  redstone) set -- 50 0 225 0 ;;
  mobs) set -- 50 0 0 300 ;;
  mixed) set -- 50 25 225 300 ;;
  mixed150) set -- 150 75 225 300 ;;
  *) echo "unknown scenario $scenario" >&2; exit 2 ;;
esac
bots=$1 wanderers=$2 clocks=$3 mobs=$4
port=19192 window=55
# Joins are spaced 500 ms (docs/swarm.md "Joins"); then 30 s for chunks and paths to settle.
warmup=$((bots / 2 + 30))
# Hyperthread pairs are n and n+6; core 5 is lent to single-core jobs.
server_cpus=4,10 swarm_cpus=0-3,6-9
mkdir -p "$out"

# Frame pointers: perf walks kernel and user stacks without debug info.
export RUSTFLAGS="-C force-frame-pointers=yes"
echo "$scenario: building"
if [ -z "${SWARM_BIN:-}" ]; then
  cargo +stable build -q --profile profiling -p acacia-bot --example swarm_load || exit 1
fi
swarm_bin=${SWARM_BIN:-target/profiling/examples/swarm_load}
(cd "$sapling" && cargo +stable build -q --release -p sapling --example load_server) || exit 1

# The box's timing convention is the bench lock, but other sessions hold it for an hour at a time.
exec 9>"$HOME/bench.lock"
locked=yes
flock -w 900 9 || locked="no (waited 900 s)"
echo "$scenario: bench lock: $locked"

RUST_LOG=info taskset -c $server_cpus "$sapling/target/release/examples/load_server" $port "$clocks" "$mobs" >"$out/server.log" 2>&1 &
server=$!
trap 'kill -INT $server 2>/dev/null' EXIT
timeout 60 bash -c "until grep -q started '$out/server.log'; do sleep 1; done" || { tail "$out/server.log"; exit 1; }
RUST_LOG=warn taskset -c $swarm_cpus "$swarm_bin" 127.0.0.1:$port "$bots" "$wanderers" $((warmup + window + 10)) >"$out/swarm.log" 2>&1 &
swarm=$!
echo "$scenario: joining, profiling in ${warmup}s"
sleep "$warmup"
ticks() { awk '{print $14 + $15}' "/proc/$1/stat"; }
ms() { echo $(( ($2 - $1) * 1000 / $(getconf CLK_TCK) )); }
swarm0=$(ticks $swarm) server0=$(ticks $server)
sudo perf stat -e task-clock,cycles,instructions,context-switches -p "$swarm" -o "$out/stat.txt" -- sleep "$window" &
stat=$!
sudo perf record -q -o "$out/perf.data" -F 1999 --call-graph fp -p "$swarm" -- sleep "$window" 2>"$out/perf.err"
wait $stat
swarm1=$(ticks $swarm) server1=$(ticks $server)
wait "$swarm"

plain() { sed 's/\x1b\[[0-9;]*m//g' "$out/server.log" | grep -o "$1.*" | head -1; }
{
  echo "scenario $scenario: $bots bots ($wanderers wandering), $clocks clocks, $mobs mobs; ${window}s window after ${warmup}s; bench lock: $locked"
  echo "spawned: $(grep -c '"event":"spawned"' "$out/swarm.log") of $bots; disconnects: $(grep -c '"event":"disconnected"' "$out/swarm.log")"
  echo "server: $(plain 'load world built') / $(plain 'lamps lit')"
  echo "swarm CPU: $(ms "$swarm0" "$swarm1") ms; server CPU: $(ms "$server0" "$server1") ms (one core = $((window * 1000)) ms)"
  echo "box load: $(cut -d' ' -f1-3 /proc/loadavg)"
} >"$out/summary.txt"
write_report "$out"
head -6 "$out/report.txt"
