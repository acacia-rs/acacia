#!/bin/bash
# Walks acacia's Java physics on Paper + Grim; prints Grim's alerts and the server's corrections.
# tools/grim-gate.sh <java_gate binary> <tp "x y z"> <fills file|-> <setup file|-> <glider 0|1> <settle s> [xkeys steps]
# Needs ~/java-truth/grim/{paper.jar,plugins/grim.jar}. Takes the shared heavy lock.
# The world gets tools/gate-datapack (functions acacia:*, e.g. the dialogs java_dialogs answers).
BIN=$1 TP=$2 FILLS=$3 SETUP=$4 GLIDER=$5 SETTLE=$6
shift 6
JDK=$HOME/java-truth/jdk/bin/java
G=$HOME/java-truth/grim
PORT=25598
LOG=$G/run/out.log
exec 9> "${HEAVY_LOCK:-/tmp/acacia-java-bridge-heavy.lock}"
flock 9
mkdir -p "$G/run/plugins"
ln -sf "$G/plugins/grim.jar" "$G/run/plugins/grim.jar"
rm -rf "$G"/run/world* "$G/run/in"
mkdir -p "$G/run/world/datapacks"
cp -r "$(dirname "$0")/gate-datapack" "$G/run/world/datapacks/acacia"
echo eula=true > "$G/run/eula.txt"
cat > "$G/run/server.properties" <<EOF
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
[ -f "$G/run/plugins/GrimAC/config.yml" ] && sed -i '/^verbose:/,/print-to-console/s/print-to-console: false/print-to-console: true/' "$G/run/plugins/GrimAC/config.yml"
: > "$LOG"
mkfifo "$G/run/in"
(cd "$G/run" && exec "$JDK" -Xmx1500M -jar "$G/paper.jar" --nogui < in > "$LOG" 2>&1 9>&-) &
server=$!
exec 8> "$G/run/in"
timeout 240 tail -n +1 -f "$LOG" | grep -m1 -E 'Done \(' > /dev/null || echo "SERVER NEVER STARTED"
echo 'forceload add -48 -48 48 48' >&8
read -t 2 <> <(:)
[ "$FILLS" != - ] && while read -r x1 y1 z1 x2 y2 z2 block; do
	[ -n "$block" ] && echo "fill $x1 $y1 $z1 $x2 $y2 $z2 $block" >&8
done < "$FILLS"
"$BIN" 127.0.0.1 "$PORT" "$SETTLE" "$GLIDER" "$@" 9>&- &
bot=$!
timeout 60 tail -n +1 -f "$LOG" | grep -m1 'joined the game' > /dev/null || echo "BOT NEVER JOINED"
echo 'kill @e[type=!player]' >&8
[ "$SETUP" != - ] && while read -r line; do
	[ -n "$line" ] && echo "$line" >&8
done < "$SETUP"
echo "tp @a $TP 0 0" >&8
tail --pid="$bot" -f /dev/null
wait "$bot"
echo "bot exit: $?"
echo stop >&8
exec 8>&-
tail --pid="$server" -f /dev/null
echo "grim alerts: $(grep -ciE 'grim.*(failed|flagged)|\[Grim\].*x[0-9]' "$LOG")"
grep -iE 'grim.*(failed|flagged)|\[Grim\].*x[0-9]|lost connection|kicked' "$LOG" | tail -n 15 | cut -c1-200
