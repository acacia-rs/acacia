# Sourced by tools/java-drills.sh and tools/java-switch.sh: Paper + Grim servers for the Java drills.
# Needs ~/java-truth/grim/{paper.jar,plugins/grim.jar}; Grim's config is copied from ~/java-truth/grim/run.
JDK=$HOME/java-truth/jdk/bin/java
G=$HOME/java-truth/grim
ALERTS='grim.*(failed|flagged)|\[Grim\].*x[0-9]'

# paper_start <dir> <port>: boots a fresh flat world in <dir> (log <dir>/out.log) and waits for it. Sets PAPER_PID
# and PAPER_FD, the console's write end. Every join gets a required resource pack, which the client must accept.
paper_start() {
	local dir=$1 port=$2
	rm -rf "$dir"/world* "$dir/in"
	mkdir -p "$dir/plugins"
	ln -sf "$G/plugins/grim.jar" "$dir/plugins/grim.jar"
	[ -d "$G/run/plugins/GrimAC" ] && [ ! -d "$dir/plugins/GrimAC" ] && cp -r "$G/run/plugins/GrimAC" "$dir/plugins/"
	echo eula=true > "$dir/eula.txt"
	printf '%s\n' online-mode=false white-list=false "server-port=$port" level-type=minecraft:flat spawn-protection=0 \
		view-distance=4 simulation-distance=4 gamemode=survival difficulty=peaceful \
		resource-pack=http://127.0.0.1:9/drill-pack.zip require-resource-pack=true > "$dir/server.properties"
	: > "$dir/out.log"
	mkfifo "$dir/in"
	(cd "$dir" && exec "$JDK" -Xmx1500M -jar "$G/paper.jar" --nogui < in > out.log 2>&1 9>&-) &
	PAPER_PID=$!
	exec {PAPER_FD}> "$dir/in"
	timeout 240 tail -n +1 -f "$dir/out.log" | grep -m1 -E 'Done \(' > /dev/null || { echo "SERVER NEVER STARTED in $dir"; exit 2; }
}

# paper_stop <pid> <fd>: stops a server from paper_start and waits for it to exit.
paper_stop() {
	echo stop >&"$2"
	eval "exec $2>&-"
	tail --pid="$1" -f /dev/null
}

# alerts_since <log> <line>: Grim alerts in <log> after line <line>.
alerts_since() {
	tail -n +"$(($2 + 1))" "$1" | grep -ciE "$ALERTS"
}
