#!/bin/sh
# usage: tools/physics/freeze.sh <trace> [from tick] [to tick]
# Per input tick: our predicted powder snow freeze count, the server's first..last count stamped with that tick,
# and the correction error there. Equal counts with an error point away from the freeze.
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
RUST_LOG=acacia_bot::movement=trace "$R/target/debug/examples/replay.exe" "$1" --correction-tolerance 0 --verbose 2>&1 |
awk -v from="${2:-0}" -v to="${3:-999999}" '
/auth input tick=/ { match($0, /tick=[0-9]+/); t = substr($0, RSTART+5, RLENGTH-5) + 0; match($0, /frozen=[0-9]+/); ours[t] = substr($0, RSTART+7, RLENGTH-7) }
/movement attribute/ { match($0, /server_tick=[0-9]+/); s = substr($0, RSTART+12, RLENGTH-12) + 0; match($0, /frozen=[0-9]+/); f = substr($0, RSTART+7, RLENGTH-7); if (!(s in first)) first[s] = f; last[s] = f }
/^corr/ { err[$2 + 0] = substr($0, index($0, "err Some(") + 9, 10) }
END { for (t = from; t <= to; t++) if (t in ours) printf "%d ours %s server %s..%s err %s\n", t, ours[t], (t in first) ? first[t] : "-", (t in last) ? last[t] : "-", (t in err) ? err[t] : "-" }'
