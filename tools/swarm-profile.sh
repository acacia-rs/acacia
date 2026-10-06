#!/usr/bin/env bash
# Runs on testbox from a synced worktree: <bots> idle swarm bots against the load server, with a perf
# profile of the swarm process once all have joined. Send it with tools/remote-run.sh.
# usage: tools/swarm-profile.sh <capture dir> [bots] [shards] [shard_fill]
#   <capture dir>: a session recorded by acacia-client's `capture` example (load_server replays it).
#   Prints the swarm's CPU over the window, then perf's self time by module and by function, then inclusive.
#   SYSCALLS=1 counts the swarm's system calls over the window instead (strace -c); STAT=1 its CPU time,
#   cycles and instructions without profiling overhead (the box idles at 800 MHz, so CPU time alone misleads).
set -uo pipefail
capture=${1:?usage: tools/swarm-profile.sh <capture dir> [bots] [shards] [shard_fill]}
bots=${2:-50}
warmup=70 window=55 port=19190
bin=target/profiling/examples

# Frame pointers: perf walks kernel and user stacks without debug info.
export RUSTFLAGS="-C force-frame-pointers=yes"
cargo +stable build -q --profile profiling -p acacia-testserver --example load_server || exit 1
cargo +stable build -q --profile profiling -p acacia-bot --example swarm_specs || exit 1

specs=$(mktemp)
python3 - "$bots" "$port" >"$specs" <<'EOF'
import json, sys
bots, port = int(sys.argv[1]), sys.argv[2]
target = {"kind": "server", "address": f"127.0.0.1:{port}"}
print(json.dumps([{"id": f"b{i}", "login": {"kind": "offline", "name": f"Load{i}"}, "target": target, "state": None} for i in range(bots)]))
EOF

"$bin/load_server" "$port" "$capture" 3 >load_server.log 2>&1 &
server=$!
trap 'kill $server 2>/dev/null; rm -f "$specs"' EXIT
timeout 120 bash -c 'until grep -q steady load_server.log; do sleep 1; done' || { cat load_server.log; exit 1; }

# The box's timing convention: the bench lock, and off the core lent to single-core jobs.
exec 9>"$HOME/bench.lock"
flock 9
RUST_LOG=warn taskset -c 0-4,6-10 "$bin/swarm_specs" "$specs" $((warmup + window + 10)) "${@:3}" >swarm.log 2>&1 &
swarm=$!
sleep "$warmup"
ticks() { awk '{print $14 + $15}' "/proc/$swarm/stat"; }
before=$(ticks)
if [ -n "${SYSCALLS:-}" ]; then
  sudo timeout "$window" strace -c -f -p "$swarm" -o syscalls.txt
  wait "$swarm"
  head -14 syscalls.txt
  exit
fi
if [ -n "${STAT:-}" ]; then
  sudo perf stat -e task-clock,cycles,instructions,context-switches -p "$swarm" -- sleep "$window" 2>&1 | grep -v '^$'
  wait "$swarm"
  exit
fi
sudo perf record -q -o perf.data -F 1999 --call-graph fp -p "$swarm" -- sleep "$window" 2>perf.err
after=$(ticks)
wait "$swarm"
report() { sudo perf report -i perf.data "$@" 2>/dev/null | grep -v '^#' | grep -v '^$' | cut -c1-150; }

echo "spawned: $(grep -c '"event":"spawned"' swarm.log) of $bots; threads with bots: $(tail -1 swarm.log | grep -o '"shard_load":\[[^]]*\]')"
echo "swarm CPU in ${window}s: $(( (after - before) * 1000 / $(getconf CLK_TCK) )) ms"
echo "== self by module"
report --no-children --sort dso | head -12
echo "== self by function"
report --no-children --sort symbol -g none | head -45
echo "== inclusive"
report --children --sort symbol -g none | head -70
