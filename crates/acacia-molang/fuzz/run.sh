#!/usr/bin/env bash
# Runs one fuzz target: fuzz/run.sh <source|differ> [seconds, default 60] [extra libFuzzer flags]
# `source` is seeded with the oracle's cases. Needs nightly and cargo-fuzz (README.md, "Fuzzing").
# Works from any directory, and in Git Bash on Windows.
set -euo pipefail

target=${1:?usage: fuzz/run.sh <source|differ> [seconds] [libFuzzer flags]}
seconds=${2:-60}
fuzz=$(cd "$(dirname "$0")" && pwd)

# On Windows the target links MSVC's AddressSanitizer runtime, which is not on PATH by default.
for asan in /c/Program\ Files*/Microsoft\ Visual\ Studio/*/*/VC/Tools/MSVC/*/bin/Hostx64/x64/clang_rt.asan_dynamic-x86_64.dll; do
    [ -e "$asan" ] && PATH="$(dirname "$asan"):$PATH"
done

corpus="$fuzz/corpus/$target"
mkdir -p "$corpus"
if [ "$target" = source ] && [ -z "$(ls -A "$corpus")" ]; then
    # One file per case; the leading byte picks the latest engine.
    grep -hv '^#' "$fuzz"/../tests/oracle/*.cases | grep . | sed 's/^truthy: //; s/ => / /' | {
        n=0
        while IFS= read -r line; do
            printf '\004%s' "$line" > "$corpus/seed-$((n += 1))"
        done
    }
fi
cargo +nightly fuzz run --fuzz-dir "$fuzz" "$target" "$corpus" -- -max_total_time="$seconds" -timeout=10 "${@:3}"
