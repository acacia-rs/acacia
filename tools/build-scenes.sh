#!/usr/bin/env bash
# Runs on testbox (through tools/remote-run.sh): examples/build.rs for each scene on a fresh world of its own
# BDS, one after another, then prints each scene's verdict line.
# usage: REMOTE_DIR=work/x tools/remote-run.sh bash tools/build-scenes.sh <bds dir> <port> <scene>...
set -uo pipefail
bds=$HOME/$1 port=$2; shift 2
here=$PWD
export PATH=$HOME/.cargo/bin:$PATH
cargo +stable build --release -q -p acacia-bot --example build || exit 1
for scene in "$@"; do
  # The server must not inherit remote-run's lock on target/.busy (fd 3), or the next job waits on it forever.
  (cd "$bds" && bash "$here/tools/bds-ctl.sh" stop && bash "$here/tools/bds-ctl.sh" reset && bash "$here/tools/bds-ctl.sh" start >/dev/null 3>&-)
  echo "== $scene"
  SCENE=$scene target/release/examples/build 127.0.0.1:$port "B$port" "O$port" >"scene-$scene.log" 2>&1
  grep -E 'PASS|FAIL|want|STALL|OPEN|outcome|Error|error|WATCHDOG' "scene-$scene.log" | head -40
done
for scene in "$@"; do grep -hE '(PASS|FAIL) after' "scene-$scene.log"; done
