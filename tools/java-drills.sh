#!/bin/bash
# Runs java_drill scenarios on one Paper + Grim server, one connection each; a drill passes with 0 Grim alerts,
# 0 setbacks and its own checks ok. tools/java-drills.sh <java_drill binary> [drill...]  (default: all)
# Needs ~/java-truth/grim/{paper.jar,plugins/grim.jar}. Takes the shared heavy lock. Exit 1 if any drill fails.
# Each drill's output lands in ~/java-truth/grim/drills/<n>-<drill>.log (DRILL_TRACE=1: the player every tick).
BIN=$1
shift
DRILLS=${*:-speed slowness jump_boost levitation slow_falling hunger powder_snow nether_lava push death chat}
JDK=$HOME/java-truth/jdk/bin/java
G=$HOME/java-truth/grim
PORT=25599
NAME=Driller
LOG=$G/drills/out.log
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
R=$G/drills
rm -rf "$R"/world* "$R/in"
mkdir -p "$R/plugins"
ln -sf "$G/plugins/grim.jar" "$R/plugins/grim.jar"
[ -d "$G/run/plugins/GrimAC" ] && [ ! -d "$R/plugins/GrimAC" ] && cp -r "$G/run/plugins/GrimAC" "$R/plugins/"
echo eula=true > "$R/eula.txt"
cat > "$R/server.properties" <<EOF
online-mode=false
white-list=false
server-port=$PORT
level-type=minecraft:flat
spawn-protection=0
view-distance=4
simulation-distance=4
gamemode=survival
difficulty=peaceful
EOF
: > "$LOG"
mkfifo "$R/in"
(cd "$R" && exec "$JDK" -Xmx1500M -jar "$G/paper.jar" --nogui < in > "$LOG" 2>&1 9>&-) &
server=$!
exec 8> "$R/in"
timeout 240 tail -n +1 -f "$LOG" | grep -m1 -E 'Done \(' > /dev/null || { echo "SERVER NEVER STARTED"; exit 2; }
echo 'forceload add 0 -16 480 16' >&8
echo "op $NAME" >&8
echo 'gamerule doImmediateRespawn false' >&8
ALERTS='grim.*(failed|flagged)|\[Grim\].*x[0-9]'
failed=0
printf '%-14s %7s %9s  %s\n' drill alerts setbacks check
n=0
for drill in $DRILLS; do
	n=$((n + 1))
	from=$(wc -l < "$LOG")
	out=$(timeout 180 "$BIN" 127.0.0.1 "$PORT" "$drill" "$NAME" 9>&- 2>&1 | tee "$R/$n-$drill.log" | grep '^DRILL ')
	read -t 2 <> <(:)
	alerts=$(tail -n +"$((from + 1))" "$LOG" | grep -ciE "$ALERTS")
	setbacks=$(sed -n 's/^DRILL [^ ]* setbacks \([0-9]*\) .*/\1/p' <<< "$out")
	check=$(sed -n 's/^DRILL [^ ]* setbacks [0-9]* check //p' <<< "$out")
	[ -z "$out" ] && check="no result"
	printf '%-14s %7s %9s  %s\n' "$drill" "$alerts" "${setbacks:--}" "$check"
	[ "$alerts" = 0 ] && [ "$setbacks" = 0 ] && [ "$check" = ok ] || {
		failed=1
		tail -n +"$((from + 1))" "$LOG" | grep -iE "$ALERTS|kicked|lost connection" | head -n 5 | cut -c1-200
	}
done
echo stop >&8
exec 8>&-
tail --pid="$server" -f /dev/null
exit $failed
