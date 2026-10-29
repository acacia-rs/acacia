#!/usr/bin/env bash
# Saves real vanilla builds (village houses, an outpost, a ruined portal, a mansion slice) from a generated BDS
# world as .mcstructure files for the builder trial (research/schematic-trial.md). Mojang content: never commit
# the output.
# usage: tools/save-vanilla-structures.sh [out dir, default ~/work/schematics/vanilla]
# Env: GEN_DIR (default ~/work/bds-trial-gen, copied from pool instance 0 if missing), GEN_PORT (19520).
# Boxes were found by saving 64x91x64 regions around /locate answers and clustering built blocks.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$HOME/work/schematics/vanilla}
gen=${GEN_DIR:-$HOME/work/bds-trial-gen}
port=${GEN_PORT:-19520}
seed=20261028

# name x0 y0 z0 x1 y1 z1 (world blocks, inclusive) on seed 20261028, BDS 1.26.52.3
boxes="
plains_house      792  65 -1955  800  74 -1947
plains_smith     3065  68 -1474 3075  75 -1467
savanna_house   -2891  67 -3004 -2881  73 -2996
desert_house     3054  67 -3027 3065  74 -3019
taiga_house       628  63  2196  636  68  2204
snowy_taiga_house  37  88  2351   43  99  2357
pillager_outpost  240  87   674  254 109   688
ruined_portal   -1094  63  -944 -1088  69  -936
mansion_slice   -1026 141 -3040 -1011 161 -3025
"

if [ ! -x "$gen/bedrock_server" ]; then
  cp -a "$HOME/work/bds-pool/0" "$gen" || exit 1
  rm -rf "$gen/worlds/actions"
fi
# The pool template is a flat world; this one generates terrain on a fixed seed.
sed -i -E "s/^level-name=.*/level-name=trialgen/; s/^level-type=.*/level-type=DEFAULT/; s/^level-seed=.*/level-seed=$seed/;
  s/^server-port=.*/server-port=$port/; s/^server-portv6=.*/server-portv6=$((port + 1))/" "$gen/server.properties"
(cd "$gen" && bash "$here/bds-ctl.sh" start >/dev/null) || exit 1

cmd() { echo "$1" >"$gen/console"; }
# Waits up to $2 seconds for a log line matching $1 after line $3.
await() { for _ in $(seq $(($2 * 5))); do tail -n +"$3" "$gen/bds.log" | grep -q "$1" && return 0; sleep 0.2; done; return 1; }

names=()
while read -r name x0 y0 z0 x1 y1 z1; do
  [ -n "$name" ] || continue
  names+=("$name")
  cmd "tickingarea remove_all"
  cmd "tickingarea add $x0 0 $z0 $x1 0 $z1 $name"
  # A save before the area's chunks have generated fails; retry until BDS confirms it.
  for try in $(seq 30); do
    n=$(wc -l <"$gen/bds.log")
    cmd "structure save $name $x0 $y0 $z0 $x1 $y1 $z1 false disk"
    await "Saved a structure with name mystructure:$name" 2 "$n" && break
    sleep 1
  done
  echo "$name saved after $try tries"
done <<<"$boxes"
cmd "tickingarea remove_all"
# The saves sit in the LevelDB log until flushed; stopping the server flushes them.
(cd "$gen" && bash "$here/bds-ctl.sh" stop)
mkdir -p "$out"
python3 -I "$here/ldb-structures.py" "$gen/worlds/trialgen/db" "$out" "${names[@]}"
