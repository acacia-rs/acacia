#!/usr/bin/env bash
# Syncs the worktree to testbox and runs a command there (default dir work/<folder>); see remote-test.sh.
# usage: REMOTE_DIR=work/bc-livetest tools/remote-run.sh cargo run --release -p acacia-bot --example actions -- 127.0.0.1:19170
# `cargo` runs the stable toolchain. Prints the last $TAIL (default 80) lines of run.log, which stays on testbox.
# Live runs and tests in the same REMOTE_DIR clobber each other (remote-test.sh clears the sources and logs).
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/$(basename "$root")}
host=${REMOTE_HOST:-testbox}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
retry() { for i in 1 2 3 4 5; do "$@" && return 0; echo "retry $i: $1" >&2; sleep 3; done; return 1; }

retry ssh $opts $host "bash -s -- $remote" <"$root/tools/testbox-target.sh" || exit 1
# A second launch into a busy dir would delete the running job's log; refuse instead.
if ssh $opts $host "test -f $remote/run.log && ! test -f $remote/run.done"; then
  echo "a job is still running in $remote (no run.done); wait for it or use another REMOTE_DIR" >&2; exit 3
fi
# Sources are dropped first, so a file deleted or turned into a directory module here goes there too
# (tar restores the rest with their mtimes: nothing rebuilds for it).
# Baked looks stay out (hundreds of MB): testbox bakes its own.
tar czf - -C "$root" --exclude=target --exclude=./assets/looks --exclude=.testserver --exclude=.tokens --exclude=.git . \
  | retry ssh $opts $host "find $remote/crates $remote/tools -name '*.rs' -delete 2>/dev/null; tar xzf - -C $remote" || exit 1
[ "${1:-}" = cargo ] && set -- cargo +stable "${@:2}"
printf 'export PATH=$HOME/.cargo/bin:$PATH RUSTC_WRAPPER=$(command -v sccache)\n%s\n' "$(printf '%q ' "$@")" \
  | retry ssh $opts $host "cat > $remote/run.sh" || exit 1
# TIMEOUT (default 1200 s) and STALL (default 300 s without output) kill a hung run: see testbox-guard.sh.
retry ssh $opts $host "cd $remote || exit; rm -f run.log run.done; { nohup flock target/.busy sh tools/testbox-guard.sh run.sh run.log run.done ${TIMEOUT:-1200} ${STALL:-300} </dev/null >/dev/null 2>&1 & }" || exit 1
# DETACH=1: leave it running; check $remote/run.done later (local polls get reaped on low-memory machines).
[ -n "${DETACH:-}" ] && { echo "launched in $remote"; exit 0; }
# Poll with short calls; a single long ssh would die with the link.
until ssh $opts $host "test -f $remote/run.done" 2>/dev/null; do sleep 5; done
retry ssh $opts $host "cd $remote; tail -n ${TAIL:-80} run.log; echo exit: \$(cat run.done)"
