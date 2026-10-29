#!/usr/bin/env bash
# Runs examples/entity_drill on testbox against a strict BDS and prints per-phase corrections.
# Usage: tools/entity-drill.sh [port] [phases,comma,separated]   (detaches; `tools/entity-drill.sh wait` blocks until done)
set -euo pipefail
DIR=work/$(basename "$(git rev-parse --show-toplevel)")
if [ "${1:-}" = wait ]; then
    ssh testbox "timeout 2500 bash -c 'until test -f $DIR/run.done; do sleep 15; done'; cd $DIR; echo exit \$(cat run.done);
        grep -E 'corrections|error|panick|WATCHDOG' run.log; grep -E 'movement correction' run.log | sed 's/\x1b\[[0-9;]*m//g' | cut -c1-260 | head -20"
    exit
fi
PORT=${1:-19188}
PHASES=${2:-}
REMOTE_DIR=$DIR DETACH=1 TIMEOUT=2400 STALL=900 bash tools/remote-run.sh bash -c \
    "RUST_LOG=acacia_bot::movement=debug DRILL_OBSERVE=Watcher DRILL_LOG=drill.log cargo +stable run --release -q -p acacia-bot --example entity_drill -- 127.0.0.1:$PORT Pusher $PHASES 2>&1 | grep -v ' trace\|rewound' | tail -400"
