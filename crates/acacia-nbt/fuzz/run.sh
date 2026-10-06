#!/usr/bin/env bash
# Runs one fuzz target, seeded with the real documents: fuzz/run.sh <decode|tree> [seconds, default 60]
# Needs nightly and cargo-fuzz (README.md, "Fuzzing"). Works from any directory, and in Git Bash on Windows.
set -euo pipefail

target=${1:?usage: fuzz/run.sh <decode|tree> [seconds]}
seconds=${2:-60}
fuzz=$(cd "$(dirname "$0")" && pwd)

# On Windows the target links MSVC's AddressSanitizer runtime, which is not on PATH by default.
for asan in /c/Program\ Files*/Microsoft\ Visual\ Studio/*/*/VC/Tools/MSVC/*/bin/Hostx64/x64/clang_rt.asan_dynamic-x86_64.dll; do
    [ -e "$asan" ] && PATH="$(dirname "$asan"):$PATH"
done

mkdir -p "$fuzz/corpus/$target"
cargo +nightly fuzz run --fuzz-dir "$fuzz" "$target" "$fuzz/corpus/$target" "$fuzz/../corpus/network" "$fuzz/../corpus/le" \
    -- -max_total_time="$seconds"
