#!/usr/bin/env bash
# Runs tools/parity/parity.py on devbox from another machine: syncs this worktree there (tools/remote-run.sh)
# and prints the result. usage: tools/parity/remote.sh [parity.py arguments]   e.g. tools/parity/remote.sh bake
# REMOTE_HOST picks another Linux x86-64 host with an oracle home (python3 tools/parity/setup.py).
root=$(cd "$(dirname "$0")/../.." && pwd)
REMOTE_HOST=${REMOTE_HOST:-devbox} TAIL=${TAIL:-60} exec "$root/tools/remote-run.sh" python3 tools/parity/parity.py "$@"
