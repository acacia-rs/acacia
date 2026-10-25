"""Light engine: every cell's block and sky light after Java's LevelLightEngine lights generated layouts (2x1 chunks, 32 high) and takes block edits, against the renderer's engine under Java's rules with the game's light facts."""
import json
import random

OWNER = "light (Java look world light)"

PALETTE = [
    "stone", "glass", "tinted_glass", "oak_leaves", "water", "lava", "ice", "slime_block", "honey_block", "soul_sand", "mud",
    "oak_slab[type=bottom]", "oak_slab[type=top]", "oak_slab[type=double]", "oak_slab[type=bottom,waterlogged=true]",
    "oak_stairs[facing=north,half=bottom,shape=straight]", "oak_stairs[facing=east,half=top,shape=outer_left]",
    "oak_stairs[facing=south,half=bottom,shape=inner_right]", "oak_stairs[facing=west,half=top,shape=straight]",
    "snow[layers=1]", "snow[layers=4]", "snow[layers=8]", "farmland", "dirt_path", "daylight_detector", "lectern",
    "stonecutter", "enchanting_table", "end_portal_frame[facing=north]", "sculk_sensor", "chest", "cobweb",
    "oak_fence[north=true,south=true]", "iron_bars[east=true,west=true]", "glass_pane[north=true,south=true]",
    "oak_trapdoor[half=top,open=false]", "oak_trapdoor[facing=east,open=true]", "oak_door[half=lower,facing=north]",
    "glowstone", "torch", "lantern", "sea_lantern", "jack_o_lantern", "redstone_lamp[lit=true]", "magma_block",
    "campfire[lit=true]", "beacon", "conduit", "shroomlight", "end_rod[facing=up]",
]
EMITTERS = ["glowstone", "torch", "lantern", "sea_lantern", "redstone_lamp[lit=true]", "lava", "shroomlight"]
CHUNKS = (2, 1)
HEIGHT = 32
CASES = 160


def layout(rng, i):
    w, d = CHUNKS[0] * 16, CHUNKS[1] * 16
    blocks = {}
    for x in range(w):
        for z in range(d):
            for y in range(rng.randrange(0, 5)):
                blocks[(x, y, z)] = "stone"
    for _ in range(rng.randrange(1, 5)):
        # A roof: shade the sky and keep block light in.
        x0, z0, y = rng.randrange(w - 4), rng.randrange(d - 4), rng.randrange(8, HEIGHT - 2)
        state = rng.choice(PALETTE[:20])
        for x in range(x0, min(w, x0 + rng.randrange(3, 14))):
            for z in range(z0, min(d, z0 + rng.randrange(3, 12))):
                if rng.random() < 0.9:
                    blocks[(x, y, z)] = state
    for _ in range(rng.randrange(50, 400)):
        blocks[(rng.randrange(w), rng.randrange(HEIGHT), rng.randrange(d))] = rng.choice(PALETTE)
    for _ in range(rng.randrange(1, 8)):
        blocks[(rng.randrange(w), rng.randrange(4, HEIGHT), rng.randrange(d))] = rng.choice(EMITTERS)
    edits = []
    for _ in range(rng.randrange(0, 12)):
        state = rng.choice(["air", "air", "stone", rng.choice(EMITTERS), rng.choice(PALETTE)])
        edits.append([rng.randrange(w), rng.randrange(1, HEIGHT), rng.randrange(d), "minecraft:" + state])
    return dict(case=f"{i}", chunks=list(CHUNKS), height=HEIGHT, sky=rng.random() < 0.8,
                blocks=[[x, y, z, "minecraft:" + s] for (x, y, z), s in sorted(blocks.items())], edits=edits)


def run(h):
    rng = random.Random(263)
    path = h.out / "layouts.jsonl"
    path.write_text("".join(json.dumps(layout(rng, i)) + "\n" for i in range(CASES)), encoding="utf-8")
    oracle = h.oracle("acacia.parity.light_engine.LightEngineOracle", path, inputs=[path]) / "layouts.jsonl"
    h.read_probe(h.probe("acacia-render", "light_engine_probe", oracle))
