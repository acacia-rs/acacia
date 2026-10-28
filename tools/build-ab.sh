#!/usr/bin/env bash
# Live builder A/B on testbox: one sync and build of swarm_build, then <runs> runs of each variant at once on
# leased pool BDS instances (tools/bds-pool.sh; PORTS="a b" for fixed ports), variants interleaved so drift hits
# all alike (tools/build-ab-run.sh, run as a detached testbox job). Prints the per-variant passes, mean and spread
# when done; WAIT=0 only launches.
# usage: [REMOTE_DIR=work/bc-ab] [WAIT=0] tools/build-ab.sh <runs> <variant>...
#   variant: name:ENV=v,ENV=v   e.g. tools/build-ab.sh 3 one:BOTS=1 two:BOTS=2
# Never a patched-tick-rate BDS: timings there are not comparable. Watch: $REMOTE_DIR/run.done, build-ab/results.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
export REMOTE_DIR=${REMOTE_DIR:-work/bc-ab}
[ $# -ge 2 ] || { echo "usage: tools/build-ab.sh <runs> <variant>..." >&2; exit 2; }
# Two rounds of runs per port at ~4 min each, plus a cold build: the guard's defaults are too short.
TIMEOUT=${TIMEOUT:-7200} STALL=${STALL:-1500} DETACH=1 "$root/tools/remote-run.sh" env PORTS="${PORTS:-}" bash tools/build-ab-run.sh "$@" || exit 1
[ "${WAIT:-1}" = 0 ] && exit 0
until ssh -o ConnectTimeout=20 testbox "test -f $REMOTE_DIR/run.done" 2>/dev/null; do sleep 30; done
ssh -o ConnectTimeout=20 testbox "sed -n '/^== results/,\$p' $REMOTE_DIR/run.log; echo exit: \$(cat $REMOTE_DIR/run.done)"
