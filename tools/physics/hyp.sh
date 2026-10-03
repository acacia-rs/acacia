#!/bin/sh
# usage: tools/physics/hyp.sh "<traces>" "HYP_X=1" ["HYP_X=1 HYP_Y=1" ...]
# A/B test of a candidate rule wrapped in an env toggle: rebuilds replay, then prints each trace's correction
# mismatches without the toggle ("base") and with each setting. "" for the traces means the regression set
# (crates/acacia-bot/tests/traces). Keep the toggle's off branch byte-identical to the current rule.
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
if ! cargo build -q -j 1 -p acacia-bot --example replay --manifest-path "$R/Cargo.toml" > "$R/target/hyp-build.log" 2>&1; then
  grep -E '^error' -A6 "$R/target/hyp-build.log" | head -20; exit 1
fi
traces=${1:-$(ls "$R"/crates/acacia-bot/tests/traces/*.btrc.gz)}; shift
for t in $traces; do
  line="$(basename "$t"):"
  for e in "" "$@"; do
    n=$(env $e "$R/target/debug/examples/replay.exe" "$t" 2>/dev/null | head -1 | sed 's/.*teleport), //; s/ of .*//')
    line="$line ${e:-base}=$n"
  done
  echo "$line"
done
