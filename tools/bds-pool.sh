#!/usr/bin/env bash
# A pool of strict BDS instances on testbox (~/work/bds-pool/<i>, port 19400+2i) that live jobs lease one at a
# time by flock, so parallel agents share them without clashing. Runs on testbox (testbox is 127.0.0.1 since
# 2026-10-24, so it also runs here directly).
# usage: tools/bds-pool.sh up <n> [template dir]   copy the template's configured BDS (strict keys, pack off) n times
#        tools/bds-pool.sh status                  each instance: port, leased or free
#        tools/bds-pool.sh run <cmd...>            lease a free instance (waits for one), export BDS_DIR/BDS_PORT,
#                                                  reset and start a fresh world, run cmd, stop the server
set -uo pipefail
pool=$HOME/work/bds-pool
ctl=$(cd "$(dirname "$0")" && pwd)/bds-ctl.sh
port_of() { echo $((19400 + 2 * $1)); }

case "${1:-}" in
  up)
    n=${2:?count}; template=$HOME/${3:-work/bds-build-oneclick}
    mkdir -p "$pool"
    for ((i = 0; i < n; i++)); do
      d=$pool/$i p=$(port_of $i)
      [ -x "$d/bedrock_server" ] || cp -r "$template" "$d"
      rm -f "$d"/bds.log "$d"/*.log
      sed -i -E "s/^server-port=.*/server-port=$p/; s/^server-portv6=.*/server-portv6=$((p + 1))/" "$d/server.properties"
      echo "$i :$p"
    done ;;
  status)
    for d in "$pool"/*/; do
      i=$(basename "$d")
      if flock -n "$d.lock" true; then s=free; else s=leased; fi
      echo "$i :$(port_of "$i") $s"
    done ;;
  run)
    shift
    [ -d "$pool" ] || { echo "no pool: tools/bds-pool.sh up <n>" >&2; exit 2; }
    while true; do
      for d in "$pool"/*/; do
        d=${d%/}
        exec 9>"$d.lock"
        if flock -n 9; then
          export BDS_DIR=$d BDS_PORT=$(port_of "$(basename "$d")")
          # fd 9 (the lease) must not leak into the server, or the lease outlives the job.
          (cd "$d" && bash "$ctl" stop >/dev/null && bash "$ctl" reset && bash "$ctl" start >/dev/null 9>&- 3>&-)
          "$@"
          rc=$?
          (cd "$d" && bash "$ctl" stop >/dev/null 9>&-)
          exit $rc
        fi
        exec 9>&-
      done
      sleep 1
    done ;;
  *) sed -n '2,9p' "$0"; exit 2 ;;
esac
