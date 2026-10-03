#!/usr/bin/env bash
# Repeats the live action run on testbox: one sync and build, then tools/live-loop.sh on both BDS instances
# in parallel (fresh world per run). Streams "<instance>/<run> PASS|FAIL ..." lines, then a per-check tally.
# usage: [ONLY=a,b] [RUST_LOG=..] [LOG_GREP='regex'] [REMOTE_DIR=..] tools/live-repeat.sh <runs> [idle]
#   LOG_GREP picks pack log lines (after "ACTIONTEST ") to stream with the results.
#   Failing runs' bot and BDS logs stay on testbox in $REMOTE_DIR/live-repeat/<instance>/<run>.{bot,bds}.log.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
remote=${REMOTE_DIR:-work/$(basename "$root")}
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
runs=${1:?usage: tools/live-repeat.sh <runs> [idle]}
shift
names=(a b)
dirs=(work/bds-actions work/bds-actions2)
ports=(19170 19172)

for k in "${!names[@]}"; do
  export BDS_DIR=${dirs[$k]} BDS_PORT=${ports[$k]}
  "$root/tools/testbox-bds.sh" stop && "$root/tools/testbox-bds.sh" install >/dev/null || exit 1
done
build=$(REMOTE_DIR=$remote TAIL=3 "$root/tools/remote-run.sh" cargo build --release -p acacia-bot --example actions)
grep -q '^exit: 0' <<<"$build" || { echo "$build"; exit 1; }

env=()
for v in ONLY RUST_LOG LOG_GREP; do [ -n "${!v:-}" ] && env+=("$v=${!v}"); done
launched=()
for k in "${!names[@]}"; do
  n=$(( runs / ${#names[@]} + (k < runs % ${#names[@]} ? 1 : 0) ))
  [ "$n" -gt 0 ] || continue
  launched+=("$k")
  cmd=$(printf '%q ' env "${env[@]}" bash "$remote/tools/live-loop.sh" "${dirs[$k]}" "${ports[$k]}" "$n" "$remote/live-repeat/${names[$k]}" "$@")
  ssh $opts testbox "mkdir -p $remote/live-repeat/${names[$k]}; rm -f $remote/live-repeat/${names[$k]}/{done,results}; { nohup $cmd </dev/null >/dev/null 2>&1 & }" || exit 1
done

# One short ssh per poll: each instance's results past what was already printed, then the done count.
seen=(0 0)
all=$(mktemp)
while :; do
  query="" done_files=""
  for k in "${launched[@]}"; do
    query+="tail -n +$(( seen[k] + 1 )) $remote/live-repeat/${names[$k]}/results 2>/dev/null | sed 's|^|${names[$k]}/|';"
    done_files+=" $remote/live-repeat/${names[$k]}/done"
  done
  snap=$(ssh $opts testbox "$query ls$done_files 2>/dev/null | wc -l") || { sleep 5; continue; }
  finished=$(tail -n 1 <<<"$snap")
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    echo "$line" | tee -a "$all"
    for k in "${!names[@]}"; do [[ $line == "${names[$k]}/"* ]] && seen[k]=$(( seen[k] + 1 )); done
  done < <(sed '$d' <<<"$snap")
  [ "$finished" -ge "${#launched[@]}" ] && break
  sleep 5
done
echo "== tally"
grep -oE '(PASS|FAIL) +[a-z_]+|ERROR' "$all" | sort | uniq -c
rm -f "$all"
