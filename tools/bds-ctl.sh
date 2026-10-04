#!/usr/bin/env bash
# Runs on testbox inside a BDS instance dir: start|stop|reset. tools/testbox-bds.sh pipes it over ssh;
# tools/live-loop.sh runs it from the synced worktree.
set -uo pipefail
# PIDs matching a pattern whose cwd is this instance (several instances share the box).
mine() { for p in $(pgrep -f "$1"); do [ "$(readlink /proc/$p/cwd)" = "$PWD" ] && echo $p; done; }

case "${1:-}" in
  start)
    [ -n "$(mine '^\./bedrock_server')" ] && { echo already running; exit; }
    # Console input goes through a fifo (`testbox-bds.sh cmd`); the cat loop reopens it after each writer.
    rm -f console; mkfifo console
    nohup sh -c 'while true; do cat console; done | LD_LIBRARY_PATH=. ./bedrock_server' >bds.log 2>&1 </dev/null &
    for _ in $(seq 150); do grep -q 'Server started' bds.log && break; sleep 0.2; done
    tail -n 5 bds.log ;;
  stop)
    if [ -n "$(mine '^\./bedrock_server')" ]; then
      echo stop > console
      for _ in $(seq 100); do [ -z "$(mine '^\./bedrock_server')" ] && break; sleep 0.2; done
      kill $(mine '^\./bedrock_server') 2>/dev/null
    fi
    # The feed loop outlives a crashed server, so it is killed either way.
    kill $(mine '^sh -c while true; do cat console') 2>/dev/null; true ;;
  reset) rm -rf worlds/actions/db worlds/actions/level.dat* worlds/actions/levelname.txt ;;
  *) echo "usage: bds-ctl.sh start|stop|reset" >&2; exit 2 ;;
esac
