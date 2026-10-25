"""Fog: FogRenderer.setupFog's colour and distances over every fog type, effect, boss bar, rain fog, void darkness and render/cloud distance, against acacia-render's fog_probe."""
from paritylib.java_look import environment_json

OWNER = "atmos (fog)"


def run(h):
    dump = h.oracle("acacia.parity.fog.FogOracle")
    got = h.out / "probe.jsonl"
    h.probe("acacia-render", "fog_probe", dump / "cases.jsonl", environment_json(h), got)
    h.compare_jsonl(dump / "records.jsonl", got)
