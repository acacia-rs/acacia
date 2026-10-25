"""Lightmap: LightmapRenderStateExtractor's state and lightmap.fsh's 16x16 texture over dimension, time, weather, lightning, brightness and effects."""
import itertools
import json

OWNER = "light (Java look world light)"

TICKS = [0, 500, 730, 3000, 6000, 11270, 11900, 12205, 13140, 18000, 22860, 23500, 24100, 1000037]
WEATHER = [(0.0, 0.0), (0.4, 0.0), (1.0, 0.0), (1.0, 0.3), (1.0, 1.0)]
PLAYER = dict(gamma=0.5, night_vision=0.0, darkness=0.0, darkness_scale=1.0, age=0.0, boss=0.0, end_flash=0.0, boss_fog=False, flicker=0.0)
# The player's side, each over a few skies.
EFFECTS = [
    dict(gamma=0.0), dict(gamma=1.0), dict(gamma=0.25, flicker=0.07), dict(flicker=-0.05),
    dict(night_vision=1.0), dict(night_vision=0.55), dict(night_vision=0.8, gamma=1.0),
    dict(darkness=1.0, age=13.0), dict(darkness=0.6, age=27.5, darkness_scale=0.5), dict(darkness=1.0, age=40.0, gamma=1.0),
    dict(boss=1.0), dict(boss=0.35), dict(end_flash=0.8), dict(end_flash=0.8, boss_fog=True),
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
    expected = h.oracle("acacia.parity.lightmap.LightmapOracle", path, inputs=[path])
    h.probe("acacia-render", "lightmap_probe", path, h.out / "rust.jsonl")
    h.compare_jsonl(expected / "records.jsonl", h.out / "rust.jsonl", key="case", tolerance=1e-6)
