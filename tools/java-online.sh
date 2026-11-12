#!/bin/bash
# Microsoft sign-in, live: an online-mode Paper + Grim, joined by acacia-java-client's java_online example.
# tools/java-online.sh <java_online binary> <token dir> [seconds] [chat]. The first run prints a device code
# ("SIGN IN: open … and enter …") and waits for the account holder; later runs reuse the token dir.
# `chat` sends a signed line and /msg with secure chat enforced, then prints the server's chat log lines.
# Keep the token dir outside every repo. Takes the heavy lock; logs in ~/java-truth/grim/online.
. "$(dirname "$0")/java-paper.sh"
BIN=$1 TOKENS=$2 SECS=${3:-30} MODE=$4
R=$G/online
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
# Properties keep the last value of a repeated key.
PAPER_PROPS="online-mode=true enforce-secure-profile=true $PAPER_PROPS" paper_start "$R" 25604
server=$PAPER_PID
from=$(wc -l < "$R/out.log")
"$BIN" "$TOKENS" 127.0.0.1 25604 "$SECS" $MODE 9>&- 2>&1 | tee "$R/client.log"
ok=${PIPESTATUS[0]}
alerts=$(alerts_since "$R/out.log" "$from")
grep -E 'UUID of player|joined the game|lost connection' "$R/out.log" | tail -n 3 | cut -c1-160
bad=0
if [ -n "$MODE" ]; then
	tail -n +"$((from + 1))" "$R/out.log" | grep -iE '<|issued server command|whisper|unsigned|chat|signature|profile key|kick' | cut -c1-200
	bad=$(tail -n +"$((from + 1))" "$R/out.log" | grep -ciE 'unsigned|validation|invalid signature|out-of-order|expired|kicked')
fi
paper_stop "$server" "$PAPER_FD"
echo "online join: client exit $ok, grim alerts $alerts, chat errors $bad"
[ "$ok" = 0 ] && [ "$alerts" = 0 ] && [ "$bad" = 0 ]
