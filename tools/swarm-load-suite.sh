#!/usr/bin/env bash
# Runs tools/swarm-load.sh on testbox for each scenario, one after another, and copies the reports here.
# usage: tools/swarm-load-suite.sh <local out dir> [scenario...]   (default: every scenario)
#   SAPLING_LOCAL=<dir> is the local sapling checkout with load_server (default ../sapling-load);
#   it is synced to work/sapling-load. BASELINE=1 also runs tools/swarm-profile.sh on the hub capture.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
local_out=${1:?usage: tools/swarm-load-suite.sh <local out dir> [scenario...]}
shift
scenarios=${*:-crowd wander redstone mobs mixed mixed150}
sapling=${SAPLING_LOCAL:-$root/../sapling-load}
remote=work/acacia-swarm-load
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"

# Ignored data files go too: sapling does not build without them.
tar czf - -C "$sapling" --exclude=target --exclude=.git . \
  | ssh $opts testbox "mkdir -p work/sapling-load && find work/sapling-load/crates -name '*.rs' -delete 2>/dev/null; tar xzf - -C work/sapling-load" || exit 1

cmd="for s in $scenarios; do bash tools/swarm-load.sh \$s swarm-load/\$s || echo \"\$s failed\"; done"
[ -n "${BASELINE:-}" ] && cmd="$cmd; bash tools/swarm-profile.sh \$HOME/work/hub-hive 50 >swarm-load/replay-report.txt 2>&1"
# A fresh sapling release build on a busy box runs past remote-run's default guard.
REMOTE_DIR=$remote TAIL=${TAIL:-20} TIMEOUT=${TIMEOUT:-5400} STALL=${STALL:-2400} "$root/tools/remote-run.sh" bash -c "$cmd"

mkdir -p "$local_out"
for s in $scenarios; do
  scp -q $opts "testbox:$remote/swarm-load/$s/report.txt" "$local_out/$s.txt" || echo "no report for $s"
done
[ -n "${BASELINE:-}" ] && scp -q $opts "testbox:$remote/swarm-load/replay-report.txt" "$local_out/replay.txt"
ls -l "$local_out"
