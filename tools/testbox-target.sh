#!/usr/bin/env bash
# Runs on testbox (piped through ssh by the sync scripts): links ~/<remote dir>/target to a tmpfs dir and
# drops the least recently used targets no job holds while tmpfs has under 5 GB free (room for a build to
# grow) or the box under 8 GB available. /dev/shm is raised to 24 GB, which a reboot undoes.
# testbox's disk is two HDDs shared with other projects: on 2026-10-24 a sapling-shard test rebuild after
# one edit took 1210 s there, 47 s on tmpfs (both under ~72% full IO pressure; tmpfs with line-tables debuginfo).
# usage: ssh testbox bash -s -- <remote dir> < tools/testbox-target.sh
# Same file as sapling's tools/testbox-target.sh: both repos' targets share the tmpfs.
set -u
remote=$HOME/$1 base=/dev/shm/targets
[ "$(df -m --output=size /dev/shm | tail -1)" -lt 24000 ] && sudo -n mount -o remount,size=24G /dev/shm
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
# Cargo's incremental dir only grows (5 GB of an 11 GB target on 2026-10-24, when tmpfs filled the box's
# RAM and swap and ssh stopped answering); past 2 GB it goes, costing one slower rebuild.
incremental=$mine/debug/incremental
[ -d "$incremental" ] && [ "$(du -sm "$incremental" | cut -f1)" -gt 2048 ] && ! busy "$mine" && rm -rf "$incremental" && echo "testbox-target: dropped $(basename "$mine")'s incremental cache"
# MemAvailable leaves tmpfs pages out: they can't be reclaimed. 8 GB is left for the box's other projects.
roomy() { [ "$(df -m --output=avail /dev/shm | tail -1)" -ge 5120 ] && [ "$(awk '/^MemAvailable/ {print int($2 / 1024)}' /proc/meminfo)" -ge 8192 ]; }
for used in $(ls -tr "$base"/*/.used 2>/dev/null); do
  victim=$(dirname "$used")
  roomy && break
  [ "$victim" = "$mine" ] || busy "$victim" || { rm -rf "$victim"; echo "testbox-target: evicted $(basename "$victim")"; }
done
# Only idle targets can go: busy ones keep growing, and a build past the tmpfs then fails with ENOSPC.
roomy || echo "testbox-target: only $(df -m --output=avail /dev/shm | tail -1) MB of tmpfs free with the rest busy; a large build may fail with ENOSPC" >&2
