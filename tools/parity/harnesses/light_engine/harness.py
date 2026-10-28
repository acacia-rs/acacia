"""Light engine: every cell's block and sky light after Java's LevelLightEngine lights generated layouts (dense 2x1 chunks 32 high; sparse 3x2 chunks 96 high) and takes block edits, in every section Java stores light for, against the renderer's engine under Java's rules with the game's light facts."""
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
# Dense: every section holds blocks. Sparse: tall, with floors in some chunks only and floating
# platforms, so whole sections stay empty and Java stores no light there.
FAMILIES = [
    dict(chunks=(2, 1), height=32, cases=160, sparse=False),
    dict(chunks=(3, 2), height=96, cases=60, sparse=True),
    dict(chunks=(3, 3), height=96, cases=40, islands=True),
]


def islands(rng, i, chunks, height):
    """A floor, then platforms high above it: the sections between store no light in Java."""
    w, d = chunks[0] * 16, chunks[1] * 16
    blocks = {}
    for x in range(w):
        for z in range(d):
            for y in range(rng.randrange(1, 4)):
                blocks[(x, y, z)] = "stone"
    for _ in range(rng.randrange(1, 4)):
        x0, z0, y = rng.randrange(w - 8), rng.randrange(d - 8), rng.randrange(66, 76)
        state = rng.choice(["stone", "oak_slab[type=bottom]", "glass", "oak_leaves", "oak_slab[type=top]"])
        for x in range(x0, min(w, x0 + rng.randrange(6, 30))):
            for z in range(z0, min(d, z0 + rng.randrange(6, 30))):
                if rng.random() < 0.95:
                    blocks[(x, y, z)] = state
    for _ in range(rng.randrange(2, 10)):
        y = rng.choice([rng.randrange(3, 8), rng.randrange(64, 80)])
        blocks[(rng.randrange(w), y, rng.randrange(d))] = rng.choice(EMITTERS)
    edits = []
    for _ in range(rng.randrange(0, 6)):
        y = rng.choice([rng.randrange(0, 8), rng.randrange(64, 80), rng.randrange(30, 50)])
        edits.append([rng.randrange(w), y, rng.randrange(d), "minecraft:" + rng.choice(["air", "stone", "glowstone"])])
    return dict(case=f"{i}", chunks=list(chunks), height=height, sky=rng.random() < 0.9,
                blocks=[[x, y, z, "minecraft:" + s] for (x, y, z), s in sorted(blocks.items())], edits=edits)


def layout(rng, i, chunks, height, sparse):
    w, d = chunks[0] * 16, chunks[1] * 16
    blocks = {}
    floored = [(cx, cz) for cx in range(chunks[0]) for cz in range(chunks[1]) if not sparse or rng.random() < 0.5]
    for x in range(w):
        for z in range(d):
            if (x >> 4, z >> 4) in floored:
                for y in range(rng.randrange(0, 5)):
                    blocks[(x, y, z)] = "stone"
    for _ in range(rng.randrange(1, 5)):
        # A roof: shade the sky and keep block light in.
        x0, z0, y = rng.randrange(w - 4), rng.randrange(d - 4), rng.randrange(8, height - 2)
        state = rng.choice(PALETTE[:20])
        for x in range(x0, min(w, x0 + rng.randrange(3, 14))):
            for z in range(z0, min(d, z0 + rng.randrange(3, 12))):
                if rng.random() < 0.9:
                    blocks[(x, y, z)] = state
    scatter = (0, 16) if sparse else (0, height)
    for _ in range(rng.randrange(50, 400)):
        blocks[(rng.randrange(w), rng.randrange(*scatter), rng.randrange(d))] = rng.choice(PALETTE)
    for _ in range(rng.randrange(1, 8)):
        blocks[(rng.randrange(w), rng.randrange(4, height), rng.randrange(d))] = rng.choice(EMITTERS)
    edits = []
    for _ in range(rng.randrange(0, 12)):
        state = rng.choice(["air", "air", "stone", rng.choice(EMITTERS), rng.choice(PALETTE)])
        edits.append([rng.randrange(w), rng.randrange(0, height), rng.randrange(d), "minecraft:" + state])
    if sparse and blocks:
        # Empty a section again, or fill a lone block into an empty one.
        x, y, z = rng.choice(sorted(blocks))
        edits.append([x, y, z, "minecraft:air"])
    return dict(case=f"{i}", chunks=list(chunks), height=height, sky=rng.random() < 0.8,
                blocks=[[x, y, z, "minecraft:" + s] for (x, y, z), s in sorted(blocks.items())], edits=edits)


def run(h):
    rng = random.Random(263)
    path = h.out / "layouts.jsonl"
    rows, i = [], 0
    for f in FAMILIES:
        for _ in range(f["cases"]):
            if f.get("islands"):
                rows.append(islands(rng, i, f["chunks"], f["height"]))
            else:
                rows.append(layout(rng, i, f["chunks"], f["height"], f["sparse"]))
            i += 1
    path.write_text("".join(json.dumps(r) + "\n" for r in rows), encoding="utf-8")
    oracle = h.oracle("acacia.parity.light_engine.LightEngineOracle", path, inputs=[path]) / "layouts.jsonl"
    h.read_probe(h.probe("acacia-render", "light_engine_probe", oracle))
