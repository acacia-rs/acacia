#!/bin/bash
# The `switch` drill behind Velocity (tools/java-proxy/velocity.toml): the bot joins backend a, moves to b with
# `/server b` (a reconfiguration in play) and must walk there with 0 Grim alerts on either backend and 0 setbacks.
# tools/java-switch.sh <java_drill binary>. Needs ~/java-truth/velocity/velocity.jar (a 26.3 build). Takes the
# shared heavy lock. Exit 1 on failure; logs in ~/java-truth/velocity/{a,b,run}.
. "$(dirname "$0")/java-paper.sh"
BIN=$1
NAME=Driller
V=$HOME/java-truth/velocity
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
for backend in a b; do
	port=$([ $backend = a ] && echo 25601 || echo 25602)
	paper_start "$V/$backend" $port
	echo 'forceload add 0 -16 480 16' >&$PAPER_FD
	echo "op $NAME" >&$PAPER_FD
	eval "pid_$backend=$PAPER_PID fd_$backend=$PAPER_FD"
done
rm -rf "$V/run" && mkdir -p "$V/run"
cp "$(dirname "$0")/java-proxy/velocity.toml" "$V/run/"
(cd "$V/run" && exec "$JDK" -Xmx256M -jar ../velocity.jar < /dev/null > out.log 2>&1 9>&-) &
proxy=$!
timeout 60 tail -n +1 -f "$V/run/out.log" | grep -m1 -E 'Done \(' > /dev/null || { echo "PROXY NEVER STARTED"; exit 2; }
from_a=$(wc -l < "$V/a/out.log")
from_b=$(wc -l < "$V/b/out.log")
out=$(timeout 240 "$BIN" 127.0.0.1 25600 switch "$NAME" 9>&- 2>&1 | tee "$V/switch.log" | grep '^DRILL ')
read -t 2 <> <(:)
alerts="$(alerts_since "$V/a/out.log" "$from_a")/$(alerts_since "$V/b/out.log" "$from_b")"
joined_b=$(tail -n +"$((from_b + 1))" "$V/b/out.log" | grep -c "$NAME joined")
echo "switch: alerts a/b $alerts, joined b $joined_b, ${out:-no result}"
kill "$proxy"
paper_stop "$pid_a" "$fd_a"
paper_stop "$pid_b" "$fd_b"
tail --pid="$proxy" -f /dev/null
[ "$alerts" = 0/0 ] && [ "$joined_b" -ge 1 ] && [[ "$out" == *"setbacks 0 check ok" ]]
