#!/bin/bash
# Java swarm load baseline: swarm_load with JAVA=1 against a local Paper + Grim, under perf stat and /usr/bin/time.
# tools/java-load.sh <swarm_load binary built with --features java> <bots> <wanderers> [seconds]
# Prints one summary line: bots spawned, client CPU seconds, instructions, peak RSS, Grim alerts. Takes the heavy
# lock. Logs in ~/java-truth/grim/load.
. "$(dirname "$0")/java-paper.sh"
BIN=$1 BOTS=$2 WANDER=$3 SECS=${4:-120}
R=$G/load
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
mkdir -p "$R"
# Bukkit refuses a second join from one address within 4 s by default.
printf 'settings:\n  connection-throttle: -1\n' > "$R/bukkit.yml"
PAPER_PROPS="max-players=$((BOTS + 10))" paper_start "$R" 25603
server=$PAPER_PID
from=$(wc -l < "$R/out.log")
# GRIM_DEBUG=1: Grim prints every bot's predicted and actual movement each tick to out.log once they have joined.
if [ -n "$GRIM_DEBUG" ]; then
	(sleep 40; for i in $(seq 0 $((BOTS - 1))); do echo "grim consoledebug Load$i" >&$PAPER_FD; done) &
fi
# PERF_RECORD=1: a flat perf profile in $R/perf.data instead of the instruction count.
if [ -n "$PERF_RECORD" ]; then
	PERF=(perf record -F 499 -o "$R/perf.data")
else
	PERF=(perf stat -x, -e instructions -o "$R/perf.txt")
fi
JAVA=1 /usr/bin/time -v -o "$R/time.txt" "${PERF[@]}" \
	"$BIN" 127.0.0.1:25603 "$BOTS" "$WANDER" "$SECS" 9>&- > "$R/swarm.log" 2>&1
alerts=$(alerts_since "$R/out.log" "$from")
paper_stop "$server" "$PAPER_FD"
spawned=$(grep -c '"event":"spawned"' "$R/swarm.log")
cpu=$(awk -F': ' '/User time|System time/ { s += $2 } END { printf "%.1f", s }' "$R/time.txt")
rss=$(awk -F': ' '/Maximum resident/ { printf "%.0f", $2 / 1024 }' "$R/time.txt")
instructions=$(awk -F, '/instructions/ { printf "%.1f", $1 / 1e9 }' "$R/perf.txt")
echo "bots $BOTS wanderers $WANDER secs $SECS spawned $spawned cpu_s $cpu instructions_g $instructions peak_rss_mb $rss grim_alerts $alerts"
