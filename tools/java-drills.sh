#!/bin/bash
# Runs java_drill scenarios on one Paper + Grim server, one connection each; a drill passes with 0 Grim alerts,
# 0 setbacks and its own checks ok. tools/java-drills.sh <java_drill binary> [drill...]  (default: all but switch,
# which tools/java-switch.sh runs). Takes the shared heavy lock. Exit 1 if any drill fails.
# Each drill's output lands in ~/java-truth/grim/drills/<n>-<drill>.log (DRILL_TRACE=1: the player every tick).
. "$(dirname "$0")/java-paper.sh"
BIN=$1
shift
DRILLS=${*:-speed slowness jump_boost levitation slow_falling hunger powder_snow nether_lava push death chat crowd gravity jump_strength step_height scale_small scale_big sneaking_speed movement_efficiency friction air_drag sprint_taps riptide riptide_short minecart}
NAME=Driller
R=$G/drills
LOG=$R/out.log
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
paper_start "$R" 25599
server=$PAPER_PID
echo 'forceload add 0 -16 1088 16' >&$PAPER_FD
echo "op $NAME" >&$PAPER_FD
echo 'gamerule doImmediateRespawn false' >&$PAPER_FD
failed=0
printf '%-14s %7s %9s  %s\n' drill alerts setbacks check
n=0
for drill in $DRILLS; do
	n=$((n + 1))
	from=$(wc -l < "$LOG")
	out=$(timeout 180 "$BIN" 127.0.0.1 25599 "$drill" "$NAME" 9>&- 2>&1 | tee "$R/$n-$drill.log" | grep '^DRILL ')
	read -t 2 <> <(:)
	alerts=$(alerts_since "$LOG" "$from")
	setbacks=$(sed -n 's/^DRILL [^ ]* setbacks \([0-9]*\) .*/\1/p' <<< "$out")
	check=$(sed -n 's/^DRILL [^ ]* setbacks [0-9]* check //p' <<< "$out")
	[ -z "$out" ] && check="no result"
	printf '%-14s %7s %9s  %s\n' "$drill" "$alerts" "${setbacks:--}" "$check"
	[ "$alerts" = 0 ] && [ "$setbacks" = 0 ] && [ "$check" = ok ] || {
		failed=1
		tail -n +"$((from + 1))" "$LOG" | grep -iE "$ALERTS|kicked|lost connection" | head -n 5 | cut -c1-200
	}
done
paper_stop "$server" "$PAPER_FD"
exit $failed
