#!/bin/bash
# The Node package on Java, live: tools/java-js-gate.mjs against a local Paper + Grim. Build the package first
# (packages/acacia-bot: npm run build). tools/java-js-gate.sh [online <token dir> <username>]: offline mode by
# default; `online` runs an online-mode server with secure chat and signs in from the token dir's cached
# `<username>.java` entry. Prints the gate line, the server's chat line and Grim's alert count. Takes the heavy
# lock; logs in ~/java-truth/grim/js-gate.
. "$(dirname "$0")/java-paper.sh"
MODE=$1 TOKENS=$2 NAME=${3:-Gate}
R=$G/js-gate
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
if [ "$MODE" = online ]; then
	PAPER_PROPS="online-mode=true enforce-secure-profile=true" paper_start "$R" 25605
else
	paper_start "$R" 25605
fi
server=$PAPER_PID
from=$(wc -l < "$R/out.log")
node "$(dirname "$0")/java-js-gate.mjs" 127.0.0.1 25605 "$NAME" ${TOKENS:+"$TOKENS"} 9>&- 2>&1 | tee "$R/client.log" | grep -E '^(GATE|joined)'
ok=${PIPESTATUS[0]}
alerts=$(alerts_since "$R/out.log" "$from")
tail -n +"$((from + 1))" "$R/out.log" | grep -E 'gate hello|secure chat|lost connection|kick' | cut -c1-160
# chat() can't tell a refused line: only the server's log shows that the chat arrived.
chat=$(tail -n +"$((from + 1))" "$R/out.log" | grep -c '> gate hello')
paper_stop "$server" "$PAPER_FD"
echo "js gate (${MODE:-offline}): node exit $ok, grim alerts $alerts, chat lines $chat"
[ "$ok" = 0 ] && [ "$alerts" = 0 ] && [ "$chat" = 1 ]
