"""Shared bootstrap self-test: client level, entity render states, GUI records and frame order, lightmap and fog extractors each run headless (oracle only, no probe)."""
import json

OWNER = "parity oracle (tools/parity/java)"


def run(h):
    out = h.oracle("acacia.parity.bootstrap.BootstrapOracle")
    lines = [json.loads(l) for l in (out / "checks.jsonl").read_text().splitlines() if l.strip()]
    for check in lines:
        if not check["ok"]:
            h.diverge(check["case"], "runs", "ok", check["detail"])
    h.add(len(lines), ())
