#!/usr/bin/env bash
# Runs `cargo test` on testbox (31 GB, Linux) for machines short on RAM. Never ships .tokens.
# usage: tools/remote-test.sh [cargo test args...]   e.g. tools/remote-test.sh -p acacia-nethernet
# Each worktree gets its own remote dir (work/<folder>); REMOTE_DIR picks another, REMOTE_HOST another ssh host.
# Waits and prints the verdict (tools/testbox-wait.sh); DETACH=1 returns at once. A filtered test is quick
# enough to run in the foreground; a suite goes DETACH=1, then testbox-wait.sh in the background.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/$(basename "$root")}
host=${REMOTE_HOST:-testbox}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
# testbox's link drops connections intermittently
retry() { for i in 1 2 3 4 5; do "$@" && return 0; echo "retry $i: $1" >&2; sleep 3; done; return 1; }

# The sync below wipes the dir: a live run's logs and journals would go with it.
for k in test run; do
  if ssh $opts $host "test -f $remote/$k.log && ! test -f $remote/$k.done"; then
    echo "a $k job is still going in $remote (no $k.done): tools/testbox-wait.sh $remote $k, or use another REMOTE_DIR" >&2; exit 3
  fi
done
retry ssh $opts $host "bash -s -- $remote" <"$root/tools/testbox-target.sh" || exit 1
# Clear old sources (keep the build cache) so files deleted locally don't linger remotely.
retry ssh $opts $host "find $remote -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +"
tar czf - -C "$root" --exclude=target --exclude=./assets/looks --exclude=.testserver --exclude=.tokens --exclude=.git . \
  | retry ssh $opts $host "tar xzf - -C $remote" || exit 1
args=$(printf '%q ' "$@")
# REMOTE_ENV="A=1 B=2" sets env vars for the remote cargo (local ones aren't forwarded).
# CARGO_INCREMENTAL=1 overrides the profile's 0: an edit to acacia-bot compiled in 4 s instead of 14 s; cargo
# only builds workspace crates incrementally, so dependencies still come from sccache.
printf '%s\n' "env RUSTC_WRAPPER=\$(PATH=\$HOME/.cargo/bin:\$PATH command -v sccache) CARGO_INCREMENTAL=1 ${REMOTE_ENV:-} ~/.cargo/bin/cargo +stable test $args" | retry ssh $opts $host "cat > $remote/test.sh" || exit 1
# TIMEOUT (default 1800 s) and STALL (default 600 s without output) kill a hung run: see testbox-guard.sh.
retry ssh $opts $host "cd $remote || exit; rm -f test.log test.done; { nohup sh tools/testbox-job.sh test ${TIMEOUT:-1800} ${STALL:-600} </dev/null >/dev/null 2>&1 & }" || exit 1
# DETACH=1: leave it running (local polls get reaped on low-memory machines).
[ -n "${DETACH:-}" ] && { echo "launched in $remote; wait: tools/testbox-wait.sh $remote test"; exit 0; }
exec "$root/tools/testbox-wait.sh" "$remote" test
