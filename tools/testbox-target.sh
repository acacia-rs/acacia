#!/usr/bin/env bash
# Runs on testbox (piped through ssh by the sync scripts): links ~/<remote dir>/target to a tmpfs dir and
# frees tmpfs by dropping the least recently used targets that no job holds. testbox's disk is two HDDs
# shared with other projects: on 2026-10-24 a sapling-shard test rebuild after one edit took 1210 s there,
# 47 s on tmpfs (both under ~72% full IO pressure; tmpfs run with line-tables debuginfo).
# usage: ssh testbox bash -s -- <remote dir> < tools/testbox-target.sh   env: TMPFS_BUDGET_GB (default 12)
# Same file as sapling's tools/testbox-target.sh: both repos' targets share the budget.
set -u
remote=$HOME/$1 base=/dev/shm/targets budget=$((${TMPFS_BUDGET_GB:-12} * 1024))
mine=$base/$(echo "$1" | tr / _)
mkdir -p "$remote" "$mine/testbox"
if [ ! -L "$remote/target" ]; then
  # An old disk target is moved aside, not deleted: rm of ~100 GB would load the disk for everyone.
  stale=$HOME/stale-targets/$(basename "$mine")
  [ -e "$remote/target" ] && mkdir -p "$HOME/stale-targets" && mv "$remote/target" "$stale" && echo "testbox-target: old target moved to $stale"
  ln -s "$mine" "$remote/target"
fi
touch "$mine/.used"
busy() { ! flock -n "$1/.busy" true || ! flock -n "$1/testbox/lock" true 2>/dev/null; }
for used in $(ls -tr "$base"/*/.used 2>/dev/null); do
  victim=$(dirname "$used")
  [ "$(du -sm "$base" | cut -f1)" -le "$budget" ] && break
  [ "$victim" = "$mine" ] || busy "$victim" || { rm -rf "$victim"; echo "testbox-target: evicted $(basename "$victim")"; }
done
