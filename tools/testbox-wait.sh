#!/usr/bin/env bash
# Waits for a testbox job started by remote-test.sh / remote-run.sh to finish, prints its verdict and exits with
# the job's exit code (124 timed out, 125 stalled, 126 build lock never freed: see testbox-job.sh).
# usage: tools/testbox-wait.sh [remote dir] [test|run]   defaults: work/<this worktree>, test
# test: errors, the failing tests' output (FAIL_LINES, default 80) and the result lines; run: the last TAIL
# (default 80) lines of run.log. Start it only after the launcher returned: a launch removes the old .done.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${1:-work/$(basename "$root")} kind=${2:-test}
host=${REMOTE_HOST:-testbox}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"

until ssh $opts "$host" "test -f $remote/$kind.done" 2>/dev/null; do sleep 2; done
if [ "$kind" = test ]; then
  show="grep -E '^(testbox-(guard|job):|error(\[|:))' -A7 test.log | head -60
    sed -n '/^failures:\$/,/^failures:\$/p' test.log | head -n ${FAIL_LINES:-80}
    grep -E '^test result|^error: test failed|could not compile' test.log"
else
  show="tail -n ${TAIL:-80} run.log"
fi
out=$(ssh $opts "$host" "cd $remote && { $show; echo \"exit: \$(cat $kind.done) (done \$(date -r $kind.done +%T))\"; }")
echo "$out"
code=$(sed -nE 's/^exit: ([0-9]+).*/\1/p' <<<"$out" | tail -n 1)
exit "${code:-1}"
