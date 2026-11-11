# Per-server adapters for tools/bench100.sh (sourced). `<target>_prepare <server dir>` writes that
# server's config for a fresh world into <server dir> and sets:
#   server_cmd   argv, run with <server dir> as cwd and the console fifo on stdin
#   ready_re     log line that means it listens
#   tick_re      thread comm of the game tick (empty: the busiest thread in the steady window)
#   tp_cmd       command each bot sends to reach its cell ({x} {y} {z}); `none` = console_tp
#   console_tp   console line moving a bot ({name} {x} {y} {z}), for servers whose players can't
#   time_cmd     command bot 0 sends every 5 s whose reply carries game time; `none` = console_poll
#   console_poll console line written every 5 s whose reply bench100-summary.py parses (TPS source)
#   stop_line    console line that stops it cleanly (empty: SIGINT)
# Settings shared by all: $SEED $VIEW $SIM $PORT $MAX_PLAYERS.

bds_properties() {
  printf '%s\n' "server-name=bench100" "gamemode=creative" "difficulty=easy" "level-name=bench100" \
    "level-type=DEFAULT" "level-seed=$SEED" "online-mode=false" "allow-list=false" "allow-cheats=true" \
    "default-player-permission-level=operator" "server-port=$PORT" "server-portv6=$((PORT + 1))" \
    "view-distance=$VIEW" "tick-distance=$SIM" "max-players=$MAX_PLAYERS" "enable-lan-visibility=false" \
    "player-idle-timeout=0" "emit-server-telemetry=false" "content-log-file-enabled=false"
}

# sapling's bench_server example (sapling branch bench/100-bot): in-memory world, vanilla commands, all ops.
sapling_prepare() {
  local bin=${SAPLING_BIN:-${SAPLING:-$root/../sapling-bench100}/target/release/examples/bench_server}
  [ -x "$bin" ] || { echo "no sapling bench_server at $bin (tools/bench100.sh build sapling)" >&2; return 1; }
  bds_properties >"$1/bench.properties"
  # SERVER_WRAP: argv prefix, e.g. "heaptrack -o <file>" (samples.tsv then shows the wrapper, not the server).
  server_cmd=(${SERVER_WRAP:-} "$bin" --properties=bench.properties)
  ready_re='started'
  tick_re='^sapling-shard-0$'
  tp_cmd='tp @s {x} {y} {z}' console_tp='' time_cmd='time query gametime' console_poll='' stop_line=''
}

# Chorus (research/chorus-bench-setup.md): /tp moves only its sender, no /time, no simulation
# distance; protocol 2225 only (acacia speaks 2193), so bots can't join until one side changes.
# CHORUS_THREADS sets its pool size (default "auto", sized from the CPUs it is pinned to).
chorus_prepare() {
  local bin=${CHORUS_BIN:-/srv/agents/cache/cargo-targets/chorus/release/chorus}
  [ -x "$bin" ] || { echo "no chorus binary at $bin (see research/chorus-bench-setup.md)" >&2; return 1; }
  printf '%s\n' '[server]' "max_players = $MAX_PLAYERS" 'authentication = false' "threads = ${CHORUS_THREADS:-\"auto\"}" \
    '[network]' 'ip = "0.0.0.0"' "port = $PORT" 'transport = "raknet"' '[network.raknet]' 'encryption = false' \
    '[level]' 'name = "bench"' "seed = $SEED" "max_view_distance = $VIEW" 'autosave = false' \
    '[permissions]' 'default = "operator"' >"$1/chorus.toml"
  server_cmd=("$bin")
  ready_re='Listening for RakNet connections'
  # The Bevy loop ticks on the main thread, whose comm is the binary's name.
  tick_re='^chorus$'
  tp_cmd='tp {x} {y} {z}' console_tp='' time_cmd='none' console_poll='status performance' stop_line='stop'
}

# Stock BDS: a symlink farm of $BDS_HOME (an unpacked bedrock-server, 1.26.52 to match acacia) per run.
bds_prepare() {
  local home=${BDS_HOME:-$HOME/work/bds-mem100}
  [ -x "$home/bedrock_server" ] || { echo "no bedrock_server in $home (set BDS_HOME)" >&2; return 1; }
  cp -rs "$home/." "$1/" || return 1
  rm -rf "$1/worlds" "$1/server.properties" "$1/permissions.json" "$1/allowlist.json" "$1"/bds.log "$1"/console
  bds_properties >"$1/server.properties"
  echo '[]' >"$1/permissions.json"
  echo '[]' >"$1/allowlist.json"
  server_cmd=(env LD_LIBRARY_PATH=. ./bedrock_server)
  ready_re='Server started'
  tick_re=''
  tp_cmd='tp @s {x} {y} {z}' console_tp='' time_cmd='time query gametime' console_poll='' stop_line='stop'
}
