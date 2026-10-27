#!/usr/bin/env bash
# On testbox, from this tree (tools/netem-pacing.sh starts it): a sapling drill server on generated
# terrain with a bot holding the view open (unchanged generated columns are not kept once nobody
# sees them), a warm-up join, then a join through netem at each profile: its join_timing line and
# raknet_trace.py's reading of the capture. With BDS_PORT set, each profile also joins that BDS.
# env: SAPLING_DIR (a sapling checkout, relative to $HOME or absolute), RADIUS (8), PORT (19240;
# netem listens on PORT + 1), BDS_PORT (a running BDS, optional), RAKNET=local (build sapling
# against this tree's acacia-raknet instead of the rev it pins).
set -u
export PATH=$HOME/.cargo/bin:$PATH
sapling=${SAPLING_DIR:?a sapling checkout}
case $sapling in /*) ;; *) sapling=$HOME/$sapling ;; esac
radius=${RADIUS:-8}
port=${PORT:-19240}
proxy_port=$((port + 1))
patch=()
[ "${RAKNET:-}" = local ] && patch=(--config "patch.'https://github.com/acacia-rs/acacia'.acacia-raknet.path='$PWD/crates/acacia-raknet'")

cargo +stable build -q --release -p acacia-netem || exit 1
cargo +stable build -q --release -p acacia-bot --example join_timing || exit 1
(cd "$sapling" && cargo +stable build -q --release "${patch[@]}" -p sapling --example drill_server) || exit 1

SAPLING_DRILL_WORLD=generated "$sapling/target/release/examples/drill_server" "$port" >server.log 2>&1 &
server=$!
holder=
proxy=
trap 'kill $server $holder $proxy 2>/dev/null' EXIT
for _ in $(seq 60); do grep -q started server.log && break; sleep 1; done
grep -q started server.log || { echo "the server did not start"; tail -n 20 server.log; exit 1; }

timing=target/release/examples/join_timing
$timing 127.0.0.1:$port Holder "$radius" 3600000 3600 >holder.log 2>&1 &
holder=$!
sleep 1
echo "warm-up: $($timing 127.0.0.1:$port Warm "$radius" 2000 300)"
n=0
through() {
  local label=$1 target=$2
  shift 2
  n=$((n + 1))
  target/release/netem --listen 127.0.0.1:$proxy_port --to 127.0.0.1:$target --stats 1 --capture capture-$n.txt "$@" >netem.log 2>&1 &
  proxy=$!
  sleep 0.5
  echo "$label: $($timing 127.0.0.1:$proxy_port Timer$n "$radius" 3000 300 2>&1 | tail -n 1)"
  sleep 1.5
  kill $proxy 2>/dev/null
  wait $proxy 2>/dev/null
  proxy=
  python3 tools/netem/raknet_trace.py capture-$n.txt | tail -n +2
  sleep 2
}
run() {
  local name=$1
  shift
  through "sapling $name" "$port" "$@"
  if [ -n "${BDS_PORT:-}" ]; then
    through "bds     $name" "$BDS_PORT" "$@"
  fi
}
run clean
run "75ms each way, 10ms jitter, 2% loss" --delay 75ms --jitter 10ms --loss 2
run "25ms each way, 2 Mbit/s down" --delay 25ms --down-rate 2mbit --down-queue 256k
run "100ms each way, 3% loss, 1 Mbit/s down" --delay 100ms --jitter 20ms --loss 3 --down-rate 1mbit --down-queue 128k
