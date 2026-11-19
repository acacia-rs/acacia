#!/bin/bash
# Runs java_drill scenarios on one Paper + Grim server, one connection each; a drill passes with 0 Grim alerts,
# 0 setbacks and its own checks ok. tools/java-drills.sh <java_drill binary> [drill...]  (default: all but switch,
# which tools/java-switch.sh runs, and the opt-in powder_snow_fall, shulker_push and shulker_box_push; see
# docs/java-edition.md). Takes the shared heavy lock. Exit 1 if any drill fails.
# Each drill's output lands in ~/java-truth/grim/drills/<n>-<drill>.log (DRILL_TRACE=1: the player every tick).
. "$(dirname "$0")/java-paper.sh"
BIN=$1
shift
DRILLS=${*:-speed slowness jump_boost levitation slow_falling hunger powder_snow nether_lava push death chat crowd gravity jump_strength step_height scale_small scale_big sneaking_speed movement_efficiency friction air_drag sprint_taps riptide riptide_short rain_riptide rain_roofed rain_relit minecart cobweb berry_bush cauldron crowd_scaled baby_push slime_push camel_dash riptide_armor_stand sulfur_push camel_sit scaled_rider boat_pair boat_back_seat camel_back_seat horse_rear camel_sit_down strider_walk horse_step boat_shulker_push horse_lid_push sleep sleep_day brew hold_mine fish sign sign_place destroy}
NAME=Driller
R=$G/drills
LOG=$R/out.log
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
paper_start "$R" 25599
server=$PAPER_PID
echo 'forceload add 0 -16 1280 16' >&$PAPER_FD
echo 'forceload add 1296 -16 1700 16' >&$PAPER_FD
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
