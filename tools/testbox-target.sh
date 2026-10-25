#!/usr/bin/env bash
# Runs on the build host (piped through ssh by the sync scripts): links ~/<remote dir>/target to a dir under
# a shared base and drops the least recently used targets no job holds while the base runs short.
# testbox: the base is tmpfs (/dev/shm, raised to 24 GB, which a reboot undoes), kept 5 GB free and the box
# 8 GB available. testbox's disk is two HDDs shared with other projects: on 2026-10-24 a sapling-shard test
# rebuild after one edit took 1210 s there, 47 s on tmpfs (both under ~72% full IO pressure; tmpfs with
# line-tables debuginfo).
# A /dev/shm mounted with usrquota (devbox) fails builds with EDQUOT while df still shows free space, so
# there the base is ~/targets on disk, kept 20 GB free; targets left in /dev/shm move there when idle.
# usage: ssh <host> bash -s -- <remote dir> < tools/testbox-target.sh
# Same file as sapling's tools/testbox-target.sh: both repos' targets share the base.
set -u
remote=$HOME/$1 name=$(echo "$1" | tr / _)
shm=/dev/shm/targets
if findmnt -no OPTIONS /dev/shm | tr , '\n' | grep -qx 'usrquota\|grpquota\|prjquota'; then
  base=$HOME/targets tmpfs=
else
  base=$shm tmpfs=1
  [ "$(df -m --output=size /dev/shm | tail -1)" -lt 24000 ] && sudo -n mount -o remount,size=24G /dev/shm
fi
mine=$base/$name
mkdir -p "$remote" "$mine/testbox"
busy() { ! flock -n "$1/.busy" true || ! flock -n "$1/testbox/lock" true 2>/dev/null; }
link=$(readlink "$remote/target" 2>/dev/null)
if [ -L "$remote/target" ] && [ "$link" != "$mine" ] && [ "$(dirname "$link")" = "$shm" ] && [ -z "$tmpfs" ]; then
  # A tmpfs target on a quota host: relink to disk once no job holds it (a running build keeps its paths).
  if [ -d "$link" ] && busy "$link"; then echo "testbox-target: $link is busy; this run stays on tmpfs" >&2; mine=$link
  else rm "$remote/target"; rm -rf "$link"; echo "testbox-target: target moved from $link to $mine (tmpfs quota)"; fi
fi
if [ -z "$tmpfs" ]; then
  # The quota is per user: idle tmpfs targets of other dirs go now; their next run relinks them to disk.
  for old in "$shm"/*/; do
    old=${old%/}
    [ -d "$old" ] && [ "$old" != "$mine" ] && ! busy "$old" && rm -rf "$old" && echo "testbox-target: freed $old (tmpfs quota)"
  done
fi
if [ ! -L "$remote/target" ]; then
  # An old disk target is moved aside, not deleted: rm of ~100 GB would load the disk for everyone.
  stale=$HOME/stale-targets/$name
  [ -e "$remote/target" ] && mkdir -p "$HOME/stale-targets" && mv "$remote/target" "$stale" && echo "testbox-target: old target moved to $stale"
  ln -s "$mine" "$remote/target"
fi
touch "$mine/.used"
# Cargo's incremental dir only grows (5 GB of an 11 GB target on 2026-10-24, when tmpfs filled the box's
# RAM and swap and ssh stopped answering); past 2 GB it goes, costing one slower rebuild.
incremental=$mine/debug/incremental
[ -d "$incremental" ] && [ "$(du -sm "$incremental" | cut -f1)" -gt 2048 ] && ! busy "$mine" && rm -rf "$incremental" && echo "testbox-target: dropped $(basename "$mine")'s incremental cache"
# MemAvailable leaves tmpfs pages out: they can't be reclaimed. 8 GB is left for the box's other projects.
roomy() {
  if [ -n "$tmpfs" ]; then
    [ "$(df -m --output=avail "$base" | tail -1)" -ge 5120 ] && [ "$(awk '/^MemAvailable/ {print int($2 / 1024)}' /proc/meminfo)" -ge 8192 ]
  else
    [ "$(df -m --output=avail "$base" | tail -1)" -ge 20480 ]
  fi
}
for used in $(ls -tr "$base"/*/.used 2>/dev/null); do
  victim=$(dirname "$used")
  roomy && break
  [ "$victim" = "$mine" ] || busy "$victim" || { rm -rf "$victim"; echo "testbox-target: evicted $(basename "$victim")"; }
done
# Only idle targets can go: busy ones keep growing, and a build past the base then fails with ENOSPC.
roomy || echo "testbox-target: only $(df -m --output=avail "$base" | tail -1) MB free under $base with the rest busy; a large build may fail with ENOSPC" >&2
