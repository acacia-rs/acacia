#!/usr/bin/env bash
# Waits until the box is quiet, then runs the given bench100 targets back to back under ~/bench.lock.
# usage: tools/bench100-when-quiet.sh <out root> <target>...   QUIET_LOAD (default 8) caps the 5-min load avg.
set -uo pipefail
root=$(dirname "$0")/..
out=${1:?usage: bench100-when-quiet.sh <out root> <target>...}; shift
quiet=${QUIET_LOAD:-8}
mkdir -p "$out"
until awk -v q="$quiet" '{exit !($2 < q)}' /proc/loadavg; do sleep 60; done
for t in "$@"; do
  echo "$(date -Is) start $t load $(cut -d' ' -f1-3 /proc/loadavg)"
  flock -o "$HOME/bench.lock" bash "$root/tools/bench100.sh" run "$t" "$out/$t" >"$out/$t.log" 2>&1
  status=$?
  echo "$(date -Is) $t exit $status"
done
bash "$root/tools/bench100.sh" summary "${@/#/$out/}" >"$out/summary.md" 2>&1
echo done >"$out/run.done"
