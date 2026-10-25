"""Sky: every rendered environment attribute (each dimension's times, weather, biomes, biome blends, lightning flashes) and SkyRenderer's meshes and frames, against acacia-render's sky_probe."""
from paritylib.java_look import environment_json

OWNER = "atmos (sky)"


def run(h):
    dump = h.oracle("acacia.parity.sky.SkyOracle")
    got = h.out / "probe.jsonl"
    h.probe("acacia-render", "sky_probe", dump / "cases.jsonl", environment_json(h), got)
    h.compare_jsonl(dump / "records.jsonl", got)
