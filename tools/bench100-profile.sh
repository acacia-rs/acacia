#!/usr/bin/env bash
# Attaches CPU and off-CPU profilers to a running bench100 server from outside (no root needed).
# usage: tools/bench100-profile.sh <server pid> <run dir>   (bench100.sh starts this under PROFILE=1)
#        tools/bench100-profile.sh report <run dir>         (per-phase tables, see bench100-profile-report.py)
# The server wants frame pointers: RUSTFLAGS="-C force-frame-pointers=yes". Hardware events can't
# use CLOCK_REALTIME (not NMI-safe), so perf timestamps are monotonic; clock.txt holds the offset
# (real - monotonic). Writes <run dir>/profile/:
#   cpu.data     cycles+instructions samples with call graphs, every thread
#   stat.csv     per-thread task-clock, cycles, instructions, switches every 5 s
#   offcpu.data  every context switch of the threads matching $OFFCPU_THREADS, stacks and durations
#   tasks.tsv    1 Hz per thread: t tid comm cpu_ns runq_ns slices voluntary involuntary state
#   proc.tsv     1 Hz: t rss_kb anon_kb
#   env.txt      loadavg, PSI, cgroup CPU stats at start and end
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
if [ "${1:-}" = report ]; then exec python3 -I "$root/tools/bench100-profile-report.py" "$2"; fi
pid=$1 out=$2/profile
freq=${PROFILE_FREQ:-249}
offcpu_threads=${OFFCPU_THREADS:-^sapling-(shard|world)}
mkdir -p "$out"

env_snapshot() {
  echo "== $1 $(date +%s.%N)"
  cat /proc/loadavg /proc/pressure/cpu
  cat "/sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.stat"
}
env_snapshot start >"$out/env.txt"
python3 -Ic 'import time; print(time.time() - time.clock_gettime(time.CLOCK_MONOTONIC))' >"$out/clock.txt"

tids=''
for task in /proc/"$pid"/task/*; do
  [[ $(<"$task/comm") =~ $offcpu_threads ]] && tids+=${tids:+,}${task##*/}
done

perf record -o "$out/cpu.data" -p "$pid" -k CLOCK_MONOTONIC -e cycles,instructions -F "$freq" --call-graph fp -m 64 >"$out/cpu.log" 2>&1 &
cpu=$!
date +%s.%N >"$out/stat.start"
perf stat -p "$pid" --per-thread -I 5000 -x, -e task-clock,cycles,instructions,context-switches,cpu-migrations -o "$out/stat.csv" >"$out/stat.log" 2>&1 &
stat=$!
offcpu=''
if [ -n "$tids" ]; then
  perf record -o "$out/offcpu.data" -t "$tids" -k CLOCK_MONOTONIC -e context-switches -c 1 --switch-events --call-graph fp -m 32 >"$out/offcpu.log" 2>&1 &
  offcpu=$!
fi
python3 -I "$root/tools/bench100-profile-sample.py" "$pid" "$out" &
sampler=$!

tail --pid="$pid" -f /dev/null
# perf exits on its own when the target does; the cap only guards a perf that hangs.
for p in $cpu $stat $offcpu $sampler; do
  timeout 90 tail --pid="$p" -f /dev/null 2>/dev/null || kill -INT "$p" 2>/dev/null
done
wait $cpu $stat $offcpu $sampler 2>/dev/null
env_snapshot end >>"$out/env.txt"
