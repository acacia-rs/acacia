#!/usr/bin/env bash
# One command for an A/B of two commits: builds swarm_load at each (once per commit and harness: the
# binaries are kept on testbox in ~/work/swarm-load-bins) and runs tools/swarm-load-ab.sh there.
# usage: tools/swarm-load-revs.sh <scenario[,scenario...]> <rev A> <rev B> [rounds]   (rev `.` = this working tree)
#   Every rev runs this tree's examples/swarm_load.rs: the harness, not the code under test.
#   Side by side, one binary against itself repeats to 0.5% in instructions on crowd, redstone, mobs and
#   wander (2026-10-24); mixed does not repeat (wanderers among mobs): don't judge a change on it.
#   DETACH=1 returns once launched; the result is the tail of <remote>/run.log when run.done appears.
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
scenario=${1:?usage: tools/swarm-load-revs.sh <scenario> <rev A> <rev B> [rounds]}
remote=work/acacia-swarm-load bins=work/swarm-load-bins
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
example=crates/acacia-bot/examples/swarm_load.rs

# `.` builds in the synced tree each time; a commit is archived to work/swarm-load-src/<sha> and built once.
names=() builds=""
for rev in "$2" "$3"; do
  if [ "$rev" = . ]; then
    names+=(tree); builds+="bash tools/swarm-load.sh build . ~/$bins/tree && "
    continue
  fi
  sha=$(git -C "$root" rev-parse --short=10 "$rev^{commit}") || exit 1
  name=$sha-$(git -C "$root" hash-object "$example" | cut -c1-6)
  [ "${names[0]:-}" = "$name" ] && { names+=("$name"); continue; }
  names+=("$name")
  ssh $opts testbox "test -x $bins/$name" && continue
  src=work/swarm-load-src/$sha
  git -C "$root" archive "$sha" | ssh $opts testbox "rm -rf $src && mkdir -p $src $bins && tar xf - -C $src" || exit 1
  ssh $opts testbox "cat > $src/$example" <"$root/$example" || exit 1
  # A throwaway target: sccache keeps the dependencies, and each commit's own crates are GBs of shared tmpfs.
  builds+="CARGO_TARGET_DIR=\$PWD/target/revs bash tools/swarm-load.sh build ~/$src ~/$bins/$name; s=\$?; rm -rf target/revs; [ \$s = 0 ] && "
done
echo "A=${names[0]} B=${names[1]}"
ab="for s in ${scenario//,/ }; do echo \"== \$s\"; bash tools/swarm-load-ab.sh \$s ~/$bins/${names[0]} ~/$bins/${names[1]} ${4:-1} | grep -E '^B/A|spawned'; done"
REMOTE_DIR=$remote TAIL=${TAIL:-40} TIMEOUT=${TIMEOUT:-5400} STALL=${STALL:-2400} "$root/tools/remote-run.sh" bash -c "${builds}${ab}"
