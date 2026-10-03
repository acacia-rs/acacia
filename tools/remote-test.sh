#!/usr/bin/env bash
# Runs `cargo test` on testbox (31 GB, Linux) for machines short on RAM. Never ships .tokens.
# usage: tools/remote-test.sh [cargo test args...]   e.g. tools/remote-test.sh -p acacia-nethernet
# Concurrent runs (several worktrees) need their own REMOTE_DIR, e.g. REMOTE_DIR=work/bc-forms.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/bedrock-client}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
# testbox's link drops connections intermittently
retry() { for i in 1 2 3 4 5; do "$@" && return 0; echo "retry $i: $1" >&2; sleep 3; done; return 1; }

# Clear old sources (keep the build cache) so files deleted locally don't linger remotely.
retry ssh $opts testbox "mkdir -p $remote && find $remote -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +"
tar czf - -C "$root" --exclude=target --exclude=.testserver --exclude=.tokens --exclude=.git . \
  | retry ssh $opts testbox "tar xzf - -C $remote" || exit 1
args=$(printf '%q ' "$@")
retry ssh $opts testbox "cd $remote || exit; rm -f test.log test.done; { nohup sh -c '~/.cargo/bin/cargo +stable test $args >test.log 2>&1; echo \$? >test.done' </dev/null >/dev/null 2>&1 & }" || exit 1
# Poll with short calls; a single long ssh would die with the link.
until ssh $opts testbox "test -f $remote/test.done" 2>/dev/null; do sleep 15; done
retry ssh $opts testbox "cd $remote; grep -E '^(error|warning)|^\s+--> |test result|FAILED|panicked' test.log | head -60; echo exit: \$(cat test.done)"
