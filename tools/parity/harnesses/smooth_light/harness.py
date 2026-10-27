"""Smooth light: BlockModelLighter's per-vertex light coords and grey (smooth light, ambient occlusion, partial-face weights, directional shade) for the quads of a block in generated 5x5x5 scenes."""
import json
import random

OWNER = "light (Java look world light)"

# Cubes, partial and non-cubic faces, see-through and dark-shading blocks, emitters and emissive ones.
PALETTE = [
    "stone", "grass_block", "glass", "oak_leaves", "ice", "slime_block", "honey_block", "soul_sand", "mud",
    "dirt_path", "farmland", "glowstone", "sea_lantern", "magma_block", "redstone_lamp[lit=true]", "jack_o_lantern",
    "oak_slab[type=bottom]", "oak_slab[type=top]", "oak_slab[type=double]",
    "oak_stairs[facing=north,half=bottom,shape=straight]", "oak_stairs[facing=east,half=top,shape=outer_left]",
    "oak_stairs[facing=south,half=bottom,shape=inner_right]",
    "snow[layers=1]", "snow[layers=3]", "snow[layers=7]", "snow[layers=8]", "oak_fence[north=true,east=true]",
    "glass_pane[north=true,south=true]", "iron_bars[east=true,west=true]", "cobweb", "short_grass", "poppy",
    "torch", "wall_torch[facing=east]", "lantern[hanging=false]", "end_rod[facing=up]", "lever[face=floor,facing=north]",
    "hopper[facing=down]", "cauldron", "anvil[facing=north]", "brewing_stand", "campfire[lit=true]",
    "oak_trapdoor[half=bottom,open=false]", "oak_door[half=lower,facing=north,open=false]", "iron_chain[axis=y]",
    "white_carpet", "stone_pressure_plate", "oak_button[face=wall,facing=south]", "ladder[facing=north]",
    "bamboo[age=0,leaves=none]", "pointed_dripstone", "amethyst_cluster[facing=up]", "beacon", "spawner", "barrier",
]
CENTRE = (2, 2, 2)
SIZE = 5
CASES = 1500


def scene(rng, i):
    nether = rng.random() < 0.2
    fill = rng.choice([0.15, 0.4, 0.7])
    blocks, lights = [], []
    for x in range(SIZE):
        for y in range(SIZE):
            for z in range(SIZE):
                if (x, y, z) == CENTRE:
                    state = PALETTE[i % len(PALETTE)]
                elif rng.random() < fill:
                    state = rng.choice(PALETTE)
                else:
                    state = None
                if state:
                    blocks.append([x, y, z, "minecraft:" + state])
                if rng.random() < 0.8:
                    # Zero light and no sky are the smooth blend's special cases.
                    sky = rng.choice([0, 0, rng.randrange(16), 15])
                    lights.append([x, y, z, sky, rng.choice([0, rng.randrange(16)])])
    light = [rng.randrange(16), rng.randrange(16)]
    return dict(case=f"{i}:{PALETTE[i % len(PALETTE)]}", nether=nether, size=[SIZE] * 3, light=light, blocks=blocks, lights=lights, draw=[list(CENTRE)])


def run(h):
    rng = random.Random(26_3)
    path = h.out / "scenes.jsonl"
    path.write_text("".join(json.dumps(scene(rng, i)) + "\n" for i in range(CASES)), encoding="utf-8")
    oracle = h.oracle("acacia.parity.smooth_light.SmoothLightOracle", path, inputs=[path]) / "scenes.jsonl"
    expected = h.out / "expected.jsonl"
    with open(oracle, encoding="utf-8") as f, open(expected, "w", encoding="utf-8") as out:
        for line in f:
            row = json.loads(line)
            quads = [dict(direction=q["direction"], coords=q["coords"], grey=q["grey"]) for q in row["quads"]]
            out.write(json.dumps(dict(case=row["case"], quads=quads)) + "\n")
    h.probe("acacia-render", "smooth_light_probe", oracle, h.out / "rust.jsonl")
    h.compare_jsonl(expected, h.out / "rust.jsonl", key="case", tolerance=0)
