"""The particles corpus: Java block states (partial; the oracle fills defaults) and particle options.

Block cases put the block at CELL over a 9x9 stone floor at y 63 (or at HIGH, three blocks up,
where it reaches below itself); particle cases start one particle over that floor or in water."""

CELL = (0, 64, 0)
HIGH = (0, 66, 0)
SEEDS = [1, 42, 1234567, -987654321]
MANY = SEEDS + list(range(100, 112))


def floor(y=63, block="minecraft:stone"):
    return [(x, y, z, block) for x in range(-4, 5) for z in range(-4, 5)]


def water():
    return [(x, y, z, "minecraft:water[level=0]") for x in range(-4, 5) for z in range(-4, 5) for y in range(64, 68)]


# name: (state at the cell, cell, extra blocks, seeds, ticks)
def _blocks():
    c, h = CELL, HIGH
    side = "north=side,east=side,south=side,west=side"
    cases = {
        "torch": ("minecraft:torch", c, [], SEEDS, 40),
        "wall_torch": ("minecraft:wall_torch[facing=north]", c, [], SEEDS, 40),
        "soul_torch": ("minecraft:soul_torch", c, [], SEEDS, 40),
        "soul_wall_torch": ("minecraft:soul_wall_torch[facing=east]", c, [], SEEDS, 40),
        "copper_torch": ("minecraft:copper_torch", c, [], SEEDS, 40),
        "copper_wall_torch": ("minecraft:copper_wall_torch[facing=south]", c, [], SEEDS, 40),
        "redstone_torch": ("minecraft:redstone_torch[lit=true]", c, [], MANY, 40),
        "redstone_wall_torch": ("minecraft:redstone_wall_torch[facing=west,lit=true]", c, [], MANY, 40),
        "furnace": ("minecraft:furnace[facing=north,lit=true]", c, [], MANY, 40),
        "blast_furnace": ("minecraft:blast_furnace[facing=east,lit=true]", c, [], MANY, 40),
        "smoker": ("minecraft:smoker[facing=south,lit=true]", c, [], MANY, 40),
        "campfire": ("minecraft:campfire[lit=true,facing=north,signal_fire=false]", c, [], SEEDS, 60),
        "signal_campfire": ("minecraft:campfire[lit=true,facing=east,signal_fire=true]", c, [(0, 63, 0, "minecraft:hay_block")], SEEDS, 60),
        "soul_campfire": ("minecraft:soul_campfire[lit=true,facing=west,signal_fire=false]", c, [], SEEDS, 60),
        "nether_portal_x": ("minecraft:nether_portal[axis=x]", c, [], SEEDS, 40),
        "nether_portal_z": ("minecraft:nether_portal[axis=z]", c, [], SEEDS, 40),
        "enchanting_table": ("minecraft:enchanting_table", c,
                             [(2, 64, 0, "minecraft:bookshelf"), (0, 64, 2, "minecraft:bookshelf"), (-2, 65, 1, "minecraft:bookshelf")], SEEDS, 40),
        "end_rod_up": ("minecraft:end_rod[facing=up]", c, [], MANY, 40),
        "end_rod_north": ("minecraft:end_rod[facing=north]", c, [], MANY, 40),
        "spore_blossom": ("minecraft:spore_blossom", h, [], SEEDS, 40),
        "crying_obsidian": ("minecraft:crying_obsidian", h, [], MANY, 60),
        "wet_sponge": ("minecraft:wet_sponge", h, [], MANY, 60),
        "mycelium": ("minecraft:mycelium[snowy=false]", c, [], MANY, 40),
        "candles": ("minecraft:candle[candles=3,lit=true]", c, [], SEEDS, 40),
        "white_candle": ("minecraft:white_candle[candles=1,lit=true]", c, [], SEEDS, 40),
        "four_candles": ("minecraft:red_candle[candles=4,lit=true]", c, [], SEEDS, 40),
        "candle_cake": ("minecraft:candle_cake[lit=true]", c, [], SEEDS, 40),
        "brewing_stand": ("minecraft:brewing_stand", c, [], SEEDS, 40),
        "ender_chest": ("minecraft:ender_chest[facing=north]", c, [], SEEDS, 40),
        "end_portal": ("minecraft:end_portal", c, [], SEEDS, 40),
        "sand": ("minecraft:sand", h, [], MANY, 40),
        "red_sand": ("minecraft:red_sand", h, [], MANY, 40),
        "gravel": ("minecraft:gravel", h, [], MANY, 40),
        "suspicious_sand": ("minecraft:suspicious_sand[dusted=0]", h, [], MANY, 40),
        "suspicious_gravel": ("minecraft:suspicious_gravel[dusted=0]", h, [], MANY, 40),
        "redstone_ore": ("minecraft:redstone_ore[lit=true]", c, [], SEEDS, 40),
        "deepslate_redstone_ore": ("minecraft:deepslate_redstone_ore[lit=true]", c, [], SEEDS, 40),
        "redstone_wire_15": (f"minecraft:redstone_wire[power=15,{side}]", c, [], SEEDS, 40),
        "redstone_wire_7": ("minecraft:redstone_wire[power=7,north=side,south=side]", c, [], SEEDS, 40),
        "repeater": ("minecraft:repeater[powered=true,delay=2,facing=north]", c, [], MANY, 40),
        "lever_wall": ("minecraft:lever[powered=true,face=wall,facing=north]", c, [], MANY, 40),
        "lever_floor": ("minecraft:lever[powered=true,face=floor,facing=east]", c, [], MANY, 40),
        "respawn_anchor": ("minecraft:respawn_anchor[charges=2]", c, [], MANY, 40),
        "bubble_column_up": ("minecraft:bubble_column[drag=false]", c, [], SEEDS, 40),
        "bubble_column_down": ("minecraft:bubble_column[drag=true]", c, [], SEEDS, 40),
        "fire_on_floor": ("minecraft:fire[age=0]", c, [], SEEDS, 40),
        "fire_on_walls": ("minecraft:fire[age=3]", h, [(1, 66, 0, "minecraft:oak_planks"), (0, 66, 1, "minecraft:oak_planks"), (0, 67, 0, "minecraft:oak_log")], SEEDS, 40),
        "soul_fire": ("minecraft:soul_fire", c, [(0, 63, 0, "minecraft:soul_sand")], SEEDS, 40),
        "sculk_sensor": ("minecraft:sculk_sensor[sculk_sensor_phase=active]", c, [], MANY, 40),
        "calibrated_sculk_sensor": ("minecraft:calibrated_sculk_sensor[sculk_sensor_phase=active,facing=north]", c, [], MANY, 40),
        "potent_sulfur": ("minecraft:potent_sulfur", c, [], SEEDS, 40),
        "wither_rose": ("minecraft:wither_rose", c, [], SEEDS, 40),
        "dried_ghast": ("minecraft:dried_ghast[hydration=0,facing=north]", c, [], MANY, 40),
        "beehive": ("minecraft:beehive[honey_level=5,facing=north]", h, [], MANY, 60),
        "bee_nest": ("minecraft:bee_nest[honey_level=5,facing=south]", h, [], MANY, 60),
        "stalactite_water": ("minecraft:pointed_dripstone[thickness=tip,vertical_direction=down]", h,
                             [(0, 67, 0, "minecraft:dripstone_block"), (0, 68, 0, "minecraft:water[level=0]")], MANY, 60),
        "stalactite_lava": ("minecraft:pointed_dripstone[thickness=tip,vertical_direction=down]", h,
                            [(0, 67, 0, "minecraft:dripstone_block"), (0, 68, 0, "minecraft:lava[level=0]")], MANY, 60),
        "stalactite_dry": ("minecraft:pointed_dripstone[thickness=tip,vertical_direction=down]", h, [(0, 67, 0, "minecraft:dripstone_block")], MANY, 60),
        "short_dry_grass": ("minecraft:short_dry_grass", c, [], SEEDS, 40),
        "dead_bush": ("minecraft:dead_bush", c, [(0, 63, 0, "minecraft:red_sand")], MANY, 40),
        "open_eyeblossom": ("minecraft:open_eyeblossom", c, [], SEEDS, 40),
        "water_source": ("minecraft:water[level=0]", c, [], MANY, 40),
        "water_flowing": ("minecraft:water[level=3]", c, [], MANY, 40),
        "water_falling": ("minecraft:water[level=8]", c, [], MANY, 40),
        "lava_open": ("minecraft:lava[level=0]", c, [], MANY, 60),
        "lava_covered": ("minecraft:lava[level=0]", c, [(0, 65, 0, "minecraft:stone")], SEEDS, 40),
        "drip_water_stone": ("minecraft:water[level=0]", (0, 67, 0), [(0, 66, 0, "minecraft:stone")], MANY, 40),
        "drip_lava_stone": ("minecraft:lava[level=0]", (0, 67, 0), [(0, 66, 0, "minecraft:stone")], MANY, 40),
        "drip_water_glass": ("minecraft:water[level=0]", (0, 67, 0), [(0, 66, 0, "minecraft:glass")], SEEDS, 40),
        "drip_water_top_slab": ("minecraft:water[level=0]", (0, 67, 0), [(0, 66, 0, "minecraft:stone_slab[type=top]")], MANY, 40),
    }
    for leaves in ["oak", "birch", "spruce", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "azalea", "flowering_azalea"]:
        cases[f"{leaves}_leaves"] = (f"minecraft:{leaves}_leaves[persistent=true]", h, [], MANY, 60)
    return cases


def _scene(cell, state, extra):
    placed = {(x, y, z): s for x, y, z, s in floor() + list(extra)}
    placed[cell] = state
    return [[x, y, z, s] for (x, y, z), s in placed.items()]


TYPES = [
    "smoke", "large_smoke", "white_smoke", "flame", "soul_fire_flame", "copper_fire_flame", "small_flame", "lava",
    "dripping_water", "falling_water", "dripping_lava", "falling_lava", "landing_lava", "dripping_honey", "falling_honey",
    "landing_honey", "falling_nectar", "falling_spore_blossom", "spore_blossom_air", "dripping_obsidian_tear",
    "falling_obsidian_tear", "landing_obsidian_tear", "dripping_dripstone_water", "falling_dripstone_water",
    "dripping_dripstone_lava", "falling_dripstone_lava", "splash", "rain", "crimson_spore", "warped_spore", "ash",
    "white_ash", "portal", "reverse_portal", "enchant", "crit", "enchanted_hit", "heart", "angry_villager",
    "happy_villager", "mycelium", "poof", "explosion", "explosion_emitter", "campfire_cosy_smoke", "campfire_signal_smoke",
    "note", "end_rod", "totem_of_undying", "cherry_leaves", "pale_oak_leaves",
]
WET = ["bubble", "bubble_column_up", "sulfur_bubbles", "current_down", "underwater"]
OPTIONS = [
    {"type": "dust", "color": 0xFF3319, "scale": 1.0},
    {"type": "dust", "color": 0x2266CC, "scale": 2.5},
    {"type": "dust_color_transition", "color": 0x3399FF, "to": 0xFF8800, "scale": 1.0},
    {"type": "tinted_leaves", "color": 0x77AB2F},
]
VELOCITIES = [[0.0, 0.0, 0.0], [0.05, 0.1, -0.03]]


def cases():
    out = []
    for name, (state, cell, extra, seeds, ticks) in _blocks().items():
        for seed in seeds:
            out.append({"case": f"block/{name}/{seed}", "kind": "block", "seed": seed, "ticks": ticks, "cell": list(cell),
                        "blocks": _scene(cell, state, extra)})
    particles = [({"type": t}, False) for t in TYPES] + [({"type": t}, True) for t in WET] + [(o, False) for o in OPTIONS]
    for options, wet in particles:
        for i, v in enumerate(VELOCITIES):
            for seed in SEEDS[:3]:
                label = options["type"] + ("" if "color" not in options else f"_{options['color']:06x}")
                at = [0.5, 65.5, 0.5] if wet else [0.5, 66.0, 0.5]
                blocks = [list(b) for b in floor() + (water() if wet else [])]
                out.append({"case": f"particle/{label}/v{i}/{seed}", "kind": "particle", "seed": seed, "ticks": 60,
                            "blocks": blocks, "particle": dict(options, pos=at, vel=v)})
    return out
