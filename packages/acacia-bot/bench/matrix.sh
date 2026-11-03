#!/usr/bin/env bash
# One bench's matrix, one process at a time, reps interleaved; rows append to results/<bench>.jsonl, logs to
# results/<bench>.log, the last failing exit code (0 if none) to results/<bench>.done. Start it detached so a
# session reap can't kill it: ssh testbox "nohup bash <pkg>/bench/matrix.sh events 3 >/dev/null 2>&1 &"
# usage: matrix.sh events|memory|reads [reps]; NS overrides the bot counts, other env passes through.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$here/results
mkdir -p "$out"
bench=$1 reps=${2:-3}
case $bench in
  events) ns=${NS:-10 100 300} ;;
  memory) ns=${NS:-0 1 10 50 100 200} ;;
  reads) ns=1 ;;
  *) echo "unknown bench $bench" >&2; exit 2 ;;
esac
rm -f "$out/$bench.done"
status=0
for rep in $(seq "$reps"); do
  for n in $ns; do
    echo "== $bench $n rep $rep load $(cut -d' ' -f1-3 /proc/loadavg)" >>"$out/$bench.log"
    timeout $((n + 300)) node --expose-gc "$here/$bench.mjs" "$n" >>"$out/$bench.jsonl" 2>>"$out/$bench.log" || {
      status=$?
      echo "== failed: exit $status" >>"$out/$bench.log"
    }
  done
done
echo "$status" >"$out/$bench.done"
