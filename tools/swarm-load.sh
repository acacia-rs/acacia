#!/usr/bin/env bash
# Runs on testbox from a synced worktree: a swarm against sapling's load_server (a busy world: a crowd
# that sees itself, redstone clocks, strolling pigs) with perf over the swarm once all have joined.
# Send it with tools/remote-run.sh; tools/swarm-load-suite.sh runs every scenario and keeps the reports.
# usage: tools/swarm-load.sh <scenario> [out dir] | report <out dir> | build <src dir> <bin>
#   scenarios: crowd wander redstone mobs mixed mixed150 (table below: bots, wanderers, clocks, mobs)
#   SAPLING=<dir> is a sapling checkout with the load_server example (default ../sapling-load).
#   SWARM_BIN=<path> runs that swarm_load build and skips both builds (tools/swarm-load-ab.sh).
#   PERF=stat counts only (no perf record, no report); PORT, SERVER_CPUS, SWARM_CPUS place a second run.
#   JOIN_MS (default 100) spaces the joins; SETTLE (default 25 s) runs after the last spawn.
#   WRAP=<cmd> runs the swarm under it (WRAP="heaptrack -o <file>"); perf still attaches to the swarm.
#   LOCK=1 takes ~/bench.lock first. Instructions don't need it; cycles do (other sessions hold it for an hour).
#   Writes <out dir>/report.txt: joins, swarm and server CPU, perf stat over the window, then self time
#   by module and function, then inclusive. The server has its own cores: its CPU near one core means
#   it, not the swarm, set the load. Compare runs by instructions: the box is shared and often busy.
set -uo pipefail
report() { sudo perf report -i "$1/perf.data" "${@:2}" 2>/dev/null | grep -v '^#' | grep -v '^$' | cut -c1-160 || true; }
write_report() {
  (
    cat "$1/summary.txt"
    grep -E 'task-clock|cycles|instructions|context-switches' "$1/stat.txt" | sed 's/  */ /g'
    [ -f "$1/perf.data" ] || exit 0
    echo "== self by module"
    report "$1" --no-children --sort dso -g none | head -12
    echo "== self by function"
    report "$1" --no-children --sort symbol -g none | head -50
    echo "== inclusive"
    report "$1" --children --sort symbol -g none | head -80
  ) >"$1/report.txt"
}
# Frame pointers: perf walks kernel and user stacks without debug info. Line tables only: full debug
# info made a 370 MB binary, and testbox is disk-bound (writes wait seconds).
export RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_PROFILING_DEBUG=line-tables-only
# The swarm_load build of the workspace at <src> (default here) into <bin>, sharing this tree's target.
build_swarm() {
  local target=${CARGO_TARGET_DIR:-$PWD/target}
  (cd "$1" && CARGO_TARGET_DIR=$target cargo +stable build -q --profile profiling -p acacia-bot --example swarm_load) || return 1
  [ -z "${2:-}" ] || cp "$target/profiling/examples/swarm_load" "$2"
}
case ${1:-} in
  report) write_report "${2:?usage: tools/swarm-load.sh report <out dir>}"; exit ;;
  build) build_swarm "${2:?usage: tools/swarm-load.sh build <src dir> <bin>}" "${3:?bin}"; exit ;;
esac

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
port=${PORT:-19192} window=55 settle=${SETTLE:-25}
# Hyperthread pairs are n and n+6; core 5 is lent to single-core jobs.
server_cpus=${SERVER_CPUS:-4,10} swarm_cpus=${SWARM_CPUS:-0-3,6-9}
rm -rf "$out"; mkdir -p "$out"

if [ -z "${SWARM_BIN:-}" ]; then
  echo "$scenario: building"
  build_swarm . || exit 1
  (cd "$sapling" && cargo +stable build -q --release -p sapling --example load_server 2>/dev/null) || exit 1
fi
swarm_bin=${SWARM_BIN:-target/profiling/examples/swarm_load}

locked=no
if [ -n "${LOCK:-}" ]; then
  exec 9>"$HOME/bench.lock"
  flock -w 900 9 && locked=yes || locked="no (waited 900 s)"
fi

RUST_LOG=info taskset -c "$server_cpus" "$sapling/target/release/examples/load_server" "$port" "$clocks" "$mobs" >"$out/server.log" 2>&1 &
server=$!
# The next run may reuse the port: wait out the server's shutdown.
stop() { kill ${swarm:-} 2>/dev/null; kill -INT $server 2>/dev/null; timeout 30 tail --pid=$server -f /dev/null || kill -KILL $server; }
trap stop EXIT
timeout 60 bash -c "until grep -q started '$out/server.log'; do sleep 1; done" || { tail "$out/server.log"; exit 1; }
JOIN_MS=${JOIN_MS:-100} RUST_LOG=warn taskset -c "$swarm_cpus" ${WRAP:-} "$swarm_bin" 127.0.0.1:"$port" "$bots" "$wanderers" 3600 >"$out/swarm.log" 2>&1 &
swarm=$!
# A wrapper (WRAP=heaptrack) is the parent; perf and /proc want the swarm itself.
[ -z "${WRAP:-}" ] || { for _ in $(seq 30); do child=$(pgrep -nf "^$swarm_bin 127.0.0.1:$port") && break; sleep 1; done; swarm=${child:-$swarm}; }
spawned() { grep -c '"event":"spawned"' "$out/swarm.log"; }
start=$SECONDS
until [ "$(spawned)" -ge "$bots" ] || [ $((SECONDS - start)) -ge $((bots + 60)) ]; do sleep 1; done
joined=$((SECONDS - start))
echo "$scenario: $(spawned) joined in ${joined}s, profiling in ${settle}s"
sleep "$settle"
ticks() { awk '{print $14 + $15}' "/proc/$1/stat"; }
ms() { echo $(( ($2 - $1) * 1000 / $(getconf CLK_TCK) )); }
server0=$(ticks $server)
# Loopback UDP is box-wide; the window's deltas are this run's when nothing else is running.
udp() { awk '/^Udp:/ && $2 ~ /^[0-9]/ {print $2, $5, $3}' /proc/net/snmp; }
udp0=$(udp)
sudo perf stat -e task-clock,cycles,instructions,context-switches -p "$swarm" -o "$out/stat.txt" -- sleep "$window" &
stat=$!
if [ "${PERF:-record}" = record ]; then
  sudo perf record -q -o "$out/perf.data" -F 1999 --call-graph fp -p "$swarm" -- sleep "$window" 2>"$out/perf.err"
fi
wait $stat
server1=$(ticks $server)
udp1=$(udp)
mem() { awk '/VmHWM|VmRSS/ {printf "%s %d MB  ", $1, $2 / 1024}' "/proc/$1/status"; }

plain() { sed 's/\x1b\[[0-9;]*m//g' "$out/server.log" | grep -o "$1.*" | head -1; }
{
  echo "scenario $scenario: $bots bots ($wanderers wandering), $clocks clocks, $mobs mobs; ${window}s window ${settle}s after ${joined}s of joins; bench lock: $locked"
  echo "spawned: $(spawned) of $bots; disconnects: $(grep -c '"event":"disconnected"' "$out/swarm.log")"
  echo "server: $(plain 'load world built') / $(plain 'lamps lit')"
  echo "swarm CPU: $(awk '/task-clock/ {printf "%d", $1}' "$out/stat.txt") ms; server CPU: $(ms "$server0" "$server1") ms (one core = $((window * 1000)) ms)"
  echo "swarm memory: $(mem $swarm); server memory: $(mem $server)"
  echo "$udp0 $udp1" | awk -v w=$window '{printf "box UDP over the window: %d in / %d out datagrams/s, %d no-port\n", ($4-$1)/w, ($5-$2)/w, ($6-$3)/w}'
  echo "box load: $(cut -d' ' -f1-3 /proc/loadavg)"
} >"$out/summary.txt"
write_report "$out"
head -6 "$out/report.txt"
