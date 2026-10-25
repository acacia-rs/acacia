#!/usr/bin/env bash
# One GUI parity harness on devbox: tools/parity/gui/run.sh <harness> [cases.py args...]
#   e.g. REMOTE_HOST=devbox DETACH=1 tools/remote-run.sh bash tools/parity/gui/run.sh hud --count 2200
# Generates cases (tools/parity/<harness>/cases.py), runs the game's code on them
# (tools/parity/<harness>/*Oracle.java over tools/parity/gui/java) and acacia's
# (tools/parity/gui-probe), then names each divergence (compare.py). Exit 1 on any divergence.
set -euo pipefail
harness=$1; shift
here=$(cd "$(dirname "$0")/../../.." && pwd)
truth=${JAVA_TRUTH:-$HOME/java-truth}
version=$truth/versions/26.3
assets=${GUI_ASSETS:-$HOME/gui-assets/assets/minecraft}
out=$here/target/parity/$harness
mkdir -p "$out/classes"
if [ ! -d "$assets" ]; then
  mkdir -p "$assets/../.." && (cd "$assets/../.." && unzip -q -o "$version/client.jar" 'assets/*')
fi
cp=$(cat "$version/classpath.txt")
oracle=$(basename "$(ls "$here/tools/parity/$harness/"*Oracle.java)" .java)
"$truth/jdk/bin/javac" --release 25 -proc:none -nowarn -cp "$cp" -d "$out/classes" \
  "$here"/tools/parity/gui/java/acacia/parity/gui/*.java "$here/tools/parity/$harness/"*.java
cargo +stable build --quiet --release -p acacia-gui-probe --manifest-path "$here/Cargo.toml"
python3 "$here/tools/parity/$harness/cases.py" "$@" > "$out/inputs.jsonl"
"$truth/jdk/bin/java" -Xmx3G -cp "$out/classes:$cp" "acacia.parity.gui.$oracle" "$out/inputs.jsonl" "$out/vanilla.jsonl" > "$out/vanilla.log" 2>&1 \
  || { tail -40 "$out/vanilla.log"; exit 2; }
"$here/target/release/gui-probe" "$harness" "$assets" "$out/inputs.jsonl" "$out/rust.jsonl"
python3 "$here/tools/parity/gui/compare.py" "$out/vanilla.jsonl" "$out/rust.jsonl" --detail "${DETAIL:-3}"
