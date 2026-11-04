#!/bin/sh
# testbox side of remote-test.sh / remote-run.sh: takes the dir's build lock, then runs <kind>.sh under
# testbox-guard.sh. Until the lock is free, <kind>.log says the job is queued; a lock still held after
# <lock_wait_s> ends the job with exit 126 and names the holder, instead of a silent wait. A guard that dies
# without writing <kind>.done (killed, OOM) gets one written here, so waiters always end.
# usage (in the job dir): sh tools/testbox-job.sh <test|run> <timeout_s> <stall_s> [lock_wait_s]
kind=$1 timeout=$2 stall=$3 wait=${4:-600}
echo "testbox-job: queued: waiting for target/.busy" >"$kind.log"
# -o: a sccache server started by the build inherits an open lock fd and would hold the lock for good.
flock -o -E 126 -w "$wait" target/.busy sh tools/testbox-guard.sh "$kind.sh" "$kind.log" "$kind.done" "$timeout" "$stall"
rc=$?
[ -f "$kind.done" ] && exit 0
if [ "$rc" = 126 ]; then
  echo "testbox-job: target/.busy still held after ${wait}s by pids: $(fuser target/.busy 2>/dev/null)" >>"$kind.log"
else
  echo "testbox-job: the guard ended without a verdict (exit $rc)" >>"$kind.log"
fi
echo "$rc" >"$kind.done"
