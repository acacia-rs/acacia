#!/usr/bin/env bash
# Endstone's 100-bot benchmark (their BENCHMARK.md method) against one server: fresh world, seed
# 20261010, view distance 10, simulation distance 4; 20 s idle, then all bots join at once, each
# teleports to its own grid cell (views never share a chunk) and stands; settle until new chunks
# drop below 0.1/s per player (300-900 s), then a 90 s steady window. Server and swarm on separate
# CPU sets. Their chaos phase is not reproduced.
# usage: tools/bench100.sh run <sapling|chorus|bds> [run dir]   (default bench100/<target>-<time>)
#        tools/bench100.sh summary <run dir>...                   (side by side when several)
#        tools/bench100.sh build <swarm|sapling>                   (release builds, -j 2, under BUILD_LOCK)
#   env: BOTS (100) PORT (19470, and +1 for BDS's v6) SEED VIEW SIM IDLE (20) MIN_SETTLE (300) MAX_SETTLE (900) STEADY (90)
#        SERVER_CPUS (0-3,8-11) SWARM_CPUS (4-7,12-15) Y (200: teleport height) SPACING (32 chunks)
#        SWARM_BIN (target/release/examples/bench100) PROFILE (1: attach perf, see tools/bench100-profile.sh)
#        BENCH_SCOPE (1: run in its own systemd user scope) BENCH_MEM_HIGH (7G) BENCH_MEM_MAX (8G)
#        sapling: SAPLING (../sapling-bench100, branch bench/100-bot; its ignored data copied from
#                 sapling/) or SAPLING_BIN. chorus: CHORUS_BIN. bds: BDS_HOME (~/work/bds-mem100).
#   Per run dir: server.log, swarm.jsonl, samples.tsv (t cpu_ticks rss_kb), threads.tsv
#   (t tid cpu_ticks comm), console-polls.tsv, summary.md. Background it: a full run is 7-17 min.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/bench100-targets.sh"

BOTS=${BOTS:-100} PORT=${PORT:-19470} SEED=${SEED:-20261010} VIEW=${VIEW:-10} SIM=${SIM:-4}
IDLE=${IDLE:-20} MIN_SETTLE=${MIN_SETTLE:-300} MAX_SETTLE=${MAX_SETTLE:-900} STEADY=${STEADY:-90}
MAX_PLAYERS=$((BOTS + 10))
server_cpus=${SERVER_CPUS:-0-3,8-11} swarm_cpus=${SWARM_CPUS:-4-7,12-15}
swarm_bin=${SWARM_BIN:-$root/target/release/examples/bench100}
lock=${BUILD_LOCK:-/tmp/claude-1001/-srv-agents-work-acacia/034e608d-3cb6-420f-95d7-90cad5f696ef/scratchpad/build.lock}

build() {
  case $1 in
    swarm) flock -o "$lock" cargo build -j 2 --release --manifest-path "$root/Cargo.toml" -p acacia-bot --example bench100 ;;
    sapling) flock -o "$lock" cargo build -j 2 --release --manifest-path "${SAPLING:-$root/../sapling-bench100}/Cargo.toml" -p sapling --example bench_server ;;
    *) echo "build swarm|sapling" >&2; return 2 ;;
  esac
}

# One line per second: process CPU and RSS to samples.tsv, every thread's CPU to threads.tsv.
sample() {
  local pid=$1 out=$2 t s rest
  while [ -d "/proc/$pid" ]; do
    t=$(date +%s.%N)
    read -r s </proc/$pid/stat 2>/dev/null || break
    rest=${s##*) }
    set -- $rest
    echo "$t $((${12} + ${13})) $(awk '/VmRSS/ {print $2}' /proc/$pid/status 2>/dev/null)" >>"$out/samples.tsv"
    for task in /proc/$pid/task/*; do
      read -r s <"$task/stat" 2>/dev/null || continue
      rest=${s##*) }
      set -- $rest
      echo "$t ${task##*/} $((${12} + ${13})) $(<"$task/comm")" >>"$out/threads.tsv"
    done 2>/dev/null
    sleep 1
  done
}

# Moves each bot from the console as it spawns (servers whose players can't teleport themselves).
console_teleports() {
  tail -n +1 -F "$1/swarm.jsonl" 2>/dev/null | while read -r line; do
    case $line in
      *'"cell":'*) eval "$(sed -E 's/.*"name":"([^"]*)".*"x":(-?[0-9]+).*"y":(-?[0-9]+).*"z":(-?[0-9]+).*/c_\1="\2 \3 \4"/' <<<"$line")" ;;
      *'"event":"spawned"'*)
        local name; name=$(sed -E 's/.*"display_name":"([^"]*)".*/\1/' <<<"$line")
        local v="c_$name"; set -- ${!v}
        local cmd=${console_tp//\{name\}/$name}; cmd=${cmd//\{x\}/$1}; cmd=${cmd//\{y\}/$2}; cmd=${cmd//\{z\}/$3}
        echo "$cmd" >&8 ;;
    esac
  done
}

run() {
  local target=$1 out=${2:-bench100/$1-$(date +%Y%m%d-%H%M%S)}
  declare -F "${target}_prepare" >/dev/null || { echo "unknown target $target" >&2; return 2; }
  [ -x "$swarm_bin" ] || { echo "no swarm at $swarm_bin (tools/bench100.sh build swarm)" >&2; return 1; }
  if ss -lun | awk '{print $4}' | grep -qE ":($PORT|$((PORT + 1)))\$"; then echo "UDP port $PORT or $((PORT + 1)) is taken; set PORT" >&2; return 1; fi
  mkdir -p "$out/server" && out=$(cd "$out" && pwd)
  "${target}_prepare" "$out/server" || return 1
  printf 'target %s bots %s seed %s view %s sim %s idle %s settle %s-%s steady %s\nserver cpus %s swarm cpus %s\nbox load %s\n' \
    "$target" "$BOTS" "$SEED" "$VIEW" "$SIM" "$IDLE" "$MIN_SETTLE" "$MAX_SETTLE" "$STEADY" "$server_cpus" "$swarm_cpus" "$(cut -d' ' -f1-3 /proc/loadavg)" >"$out/meta.txt"
  printf 'tick_re %s\ntime_cmd %s\nconsole_poll %s\n' "$tick_re" "$time_cmd" "$console_poll" >>"$out/meta.txt"

  rm -f "$out/console"; mkfifo "$out/console"
  # Held read-write so the server's stdin never sees EOF between writers.
  exec 8<>"$out/console"
  # Its own process group: a SERVER_WRAP like heaptrack doesn't pass SIGINT on, which orphaned a server for 2 days.
  (cd "$out/server" && exec setsid taskset -c "$server_cpus" "${server_cmd[@]}") <"$out/console" >"$out/server.log" 2>&1 &
  # Globals: the EXIT trap runs after run() has returned.
  server=$! swarm='' helpers=()
  stop() {
    trap - EXIT INT TERM
    [ -n "$swarm" ] && kill "$swarm" 2>/dev/null
    for h in "${helpers[@]}"; do kill "$h" 2>/dev/null; done
    if [ -n "$stop_line" ]; then echo "$stop_line" >&8; else kill -INT -- "-$server" 2>/dev/null; fi
    timeout 60 tail --pid="$server" -f /dev/null || kill -KILL -- "-$server" 2>/dev/null
    pkill -KILL -g "$server" 2>/dev/null
    exec 8>&-
    scope_pressure >>"$out/meta.txt"
  }
  trap stop EXIT INT TERM
  timeout 300 bash -c "until grep -qE '$ready_re' '$out/server.log'; do kill -0 $server || exit 1; sleep 1; done" \
    || { echo "server did not start:"; tail -n 20 "$out/server.log"; return 1; }
  sample "$server" "$out" &
  helpers+=($!)
  local profile=''
  if [ "${PROFILE:-0}" = 1 ]; then bash "$root/tools/bench100-profile.sh" "$server" "$out" & profile=$!; fi
  echo "$target: up, idle ${IDLE}s"
  sleep "$IDLE"

  local args=(--radius "$VIEW" --min-settle "$MIN_SETTLE" --max-settle "$MAX_SETTLE" --steady "$STEADY" --y "${Y:-200}" --spacing "${SPACING:-32}" --tp-cmd "$tp_cmd" --time-cmd "$time_cmd")
  RUST_LOG=warn taskset -c "$swarm_cpus" "$swarm_bin" "127.0.0.1:$PORT" "$BOTS" "${args[@]}" >"$out/swarm.jsonl" 2>"$out/swarm.err" &
  swarm=$!
  if [ "$tp_cmd" = none ] && [ -n "$console_tp" ]; then console_teleports "$out" & helpers+=($!); fi
  if [ -n "$console_poll" ]; then
    (while kill -0 "$swarm" 2>/dev/null; do date +%s.%N >>"$out/console-polls.tsv"; echo "$console_poll" >&8; sleep 5; done) &
    helpers+=($!)
  fi
  echo "$target: $BOTS bots joining; settle ${MIN_SETTLE}-${MAX_SETTLE}s, steady ${STEADY}s"
  timeout $((MAX_SETTLE + STEADY + 300)) tail --pid="$swarm" -f /dev/null
  wait "$swarm" 2>/dev/null; swarm=''
  stop
  # stop() killed its helpers; the profilers end with the server and finish writing here.
  [ -n "$profile" ] && wait "$profile" 2>/dev/null
  python3 -I "$root/tools/bench100-summary.py" "$out" | tee "$out/summary.md"
}

# The memory scope's peak and how often it hit memory.high: a scope at its limit stalls every process in it,
# bots included, past RakNet's 10 s timeout (acacia reports/net-backlog-20261110).
scope_pressure() {
  local cg; cg=/sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)
  printf 'scope %s peak %s MiB, memory.high events %s\n' "${cg##*/}" "$(($(cat "$cg/memory.peak" 2>/dev/null || echo 0) >> 20))" \
    "$(awk '$1 == "high" {print $2}' "$cg/memory.events" 2>/dev/null)"
}

case ${1:-} in
  run)
    # Own scope, so the session's other jobs can't push the bench into memory.high (BENCH_SCOPE=0 to skip).
    if [ "${BENCH_SCOPE:-1}" = 1 ] && [ -z "${BENCH100_SCOPED:-}" ]; then
      exec systemd-run --user --scope -q -p MemoryHigh="${BENCH_MEM_HIGH:-7G}" -p MemoryMax="${BENCH_MEM_MAX:-8G}" \
        env BENCH100_SCOPED=1 bash "$0" "$@"
    fi
    shift; run "$@" ;;
  summary) shift; python3 -I "$root/tools/bench100-summary.py" "$@" ;;
  build) build "${2:-}" ;;
  *) sed -n '2,17p' "$0"; exit 2 ;;
esac
