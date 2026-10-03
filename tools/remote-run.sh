#!/usr/bin/env bash
# Syncs the worktree to testbox and runs a command there (default dir work/bedrock-client); see remote-test.sh.
# usage: REMOTE_DIR=work/bc-livetest tools/remote-run.sh cargo run --release -p acacia-bot --example actions -- 127.0.0.1:19170
# `cargo` runs the stable toolchain. Prints the last $TAIL (default 80) lines of run.log, which stays on testbox.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/bedrock-client}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
retry() { for i in 1 2 3 4 5; do "$@" && return 0; echo "retry $i: $1" >&2; sleep 3; done; return 1; }

retry ssh $opts testbox "mkdir -p $remote"
tar czf - -C "$root" --exclude=target --exclude=.testserver --exclude=.tokens --exclude=.git . \
  | retry ssh $opts testbox "tar xzf - -C $remote" || exit 1
[ "${1:-}" = cargo ] && set -- cargo +stable "${@:2}"
printf 'export PATH=$HOME/.cargo/bin:$PATH\n%s\n' "$(printf '%q ' "$@")" \
  | retry ssh $opts testbox "cat > $remote/run.sh" || exit 1
retry ssh $opts testbox "cd $remote || exit; rm -f run.log run.done; { nohup sh -c 'sh run.sh >run.log 2>&1; echo \$? >run.done' </dev/null >/dev/null 2>&1 & }" || exit 1
# Poll with short calls; a single long ssh would die with the link.
until ssh $opts testbox "test -f $remote/run.done" 2>/dev/null; do sleep 10; done
retry ssh $opts testbox "cd $remote; tail -n ${TAIL:-80} run.log; echo exit: \$(cat run.done)"
