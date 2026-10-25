"""Lightmap: the real client's LightmapRenderState (ClientLevel clocks, weather, lightning, End flash; LocalPlayer effects; options) and lightmap.fsh's 16x16 texture over it, against the renderer's from the same inputs."""
import itertools
import json

OWNER = "light (Java look world light)"

TICKS = [0, 500, 730, 3000, 6000, 11270, 11900, 12205, 13140, 18000, 22860, 23500, 24100, 1000037]
WEATHER = [(0.0, 0.0), (0.4, 0.0), (1.0, 0.0), (1.0, 0.3), (0.6, 1.0), (1.0, 1.0)]
PLAYER = dict(gamma=0.5, night_vision_ticks=0, darkness_ticks=-1, darkness_scale=1.0, age=0, boss=0.0, end_flash=0.0, flicker=0.0)
# The player's side, each over a few skies.
EFFECTS = [
    dict(gamma=0.0), dict(gamma=1.0), dict(gamma=0.25, flicker=0.07), dict(flicker=-0.05),
    dict(night_vision_ticks=1000), dict(night_vision_ticks=150), dict(night_vision_ticks=77, gamma=1.0),
    dict(darkness_ticks=40, age=13), dict(darkness_ticks=7, age=27, darkness_scale=0.5), dict(darkness_ticks=20, age=40, gamma=1.0),
    dict(darkness_ticks=0, age=3), dict(boss=1.0), dict(boss=0.35), dict(end_flash=0.8), dict(end_flash=0.3, gamma=0.0),
]
SKIES = [("overworld", 6000, 0.0, 0.0, False), ("overworld", 18000, 1.0, 0.5, False), ("the_nether", 0, 0.0, 0.0, False), ("the_end", 0, 0.0, 0.0, False)]


def cases():
    out = []
    for dim, ticks, (rain, thunder), flash in itertools.product(["overworld", "the_nether", "the_end"], TICKS, WEATHER, [False, True]):
        out.append(dict(PLAYER, dimension=dim, ticks=ticks, rain=rain, thunder=thunder, flash=flash))
    for effect, (dim, ticks, rain, thunder, flash) in itertools.product(EFFECTS, SKIES):
        out.append(dict(PLAYER, **effect, dimension=dim, ticks=ticks, rain=rain, thunder=thunder, flash=flash))
    for i, c in enumerate(out):
        c["case"] = f"{i}:{c['dimension']}@{c['ticks']}"
    return out


def run(h):
    path = h.out / "cases.jsonl"
    path.write_text("".join(json.dumps(c) + "\n" for c in cases()), encoding="utf-8")
    records = h.oracle("acacia.parity.lightmap.LightmapOracle", path, inputs=[path]) / "records.jsonl"
    expected = h.out / "expected.jsonl"
    with open(records, encoding="utf-8") as f, open(expected, "w", encoding="utf-8") as out:
        for line in f:
            r = json.loads(line)
            out.write(json.dumps(dict(case=r["case"], state=r["state"])) + "\n")
    h.probe("acacia-render", "lightmap_probe", records, h.out / "rust.jsonl")
    h.compare_jsonl(expected, h.out / "rust.jsonl", key="case", tolerance=1e-6)
