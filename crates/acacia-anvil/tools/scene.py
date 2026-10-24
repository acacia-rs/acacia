"""Server commands that build the oracle's test scene at y = Y over (X0, Z0): every Java block's default state
on a grid (placed `strict`, so nothing reshapes or breaks), then hand-made cases the converter must rebuild
(double chests, bed and banner colours, pots, skulls, shulkers, redstone, snowy grass, mushroom caps, kelp,
lit rooms, slab roofs), placed with updates so Java computes their shapes itself.
"""
import json
from pathlib import Path

X0, Z0, Y = 0, 0, 150
GRID = 2
DYES = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
        "purple", "blue", "brown", "green", "red", "black"]
# blocks that flow, need a partner the grid cannot give, or are not placeable
SKIP = {"minecraft:air", "minecraft:cave_air", "minecraft:void_air", "minecraft:water", "minecraft:lava",
        "minecraft:bubble_column", "minecraft:moving_piston", "minecraft:piston_head", "minecraft:fire",
        "minecraft:soul_fire", "minecraft:nether_portal", "minecraft:end_portal", "minecraft:end_gateway",
        # states no player can leave alone (a vine body with no tip, a dust dot from setblock): Bedrock has no
        # way to say them, the converter rebuilds the natural form (README.md "Known differences")
        "minecraft:kelp_plant", "minecraft:twisting_vines_plant", "minecraft:weeping_vines_plant",
        "minecraft:cave_vines_plant", "minecraft:redstone_wire"}


def grid_blocks(report: Path) -> list[str]:
    blocks = json.loads(report.read_text())
    return [name for name in blocks if name not in SKIP]


def commands(report: Path) -> list[str]:
    names = grid_blocks(report)
    side = int(len(names) ** 0.5) + 1
    span = side * GRID
    cx, cz = X0 + span // 2, Z0 + span // 2
    cmds = [
        # camelCase names before 1.21.11, snake_case after; the other form just errors
        "gamerule randomTickSpeed 0", "gamerule doFireTick false", "gamerule doDaylightCycle false",
        "gamerule doMobSpawning false", "gamerule random_tick_speed 0", "gamerule fire_spread_radius_around_player 0",
        "gamerule advance_time false", "gamerule spawn_mobs false", "time set noon", "weather clear",
        f"forceload add {X0 - 32} {Z0 - 32} {X0 + span + 64} {Z0 + span + 32}",
    ]
    # clear what an earlier run left, then the platform, in 32-wide strips (fill's volume limit)
    for x in range(X0 - 4, X0 + span + 48, 32):
        x1 = min(x + 31, X0 + span + 47)
        cmds.append(f"fill {x} {Y} {Z0 - 4} {x1} {Y + 8} {Z0 + span + 4} air strict")
        cmds.append(f"fill {x} {Y - 1} {Z0 - 4} {x1} {Y - 1} {Z0 + span + 4} smooth_stone")
    for i, name in enumerate(names):
        x, z = X0 + (i % side) * GRID, Z0 + (i // side) * GRID
        cmds.append(f"setblock {x} {Y} {z} {name} strict")
    cmds += curated(X0 + span + 8, Z0)
    cmds.append(f"setworldspawn {cx} {Y + 1} {cz}")
    return cmds


def where(report: Path, cells: list[str]) -> None:
    """Names the grid block at each `x,y,z` (diff-render's worst cells)."""
    names = grid_blocks(report)
    side = int(len(names) ** 0.5) + 1
    for cell in cells:
        x, _, z = map(int, cell.split(","))
        i = (z - Z0) // GRID * side + (x - X0) // GRID
        inside = 0 <= x - X0 < side * GRID and 0 <= i < len(names)
        print(cell, names[i] if inside else "(curated area)")


if __name__ == "__main__":
    import sys
    where(Path(__file__).resolve().parent / ".cache/java-26.3-blocks.json", sys.argv[1:])


def curated(x0: int, z0: int) -> list[str]:
    c, y = [], Y
    # double chests both ways, single chests side by side, trapped
    for i, f in enumerate(["north", "south", "east", "west"]):
        x = x0 + i * 4
        # Java's left half has its partner clockwise of its facing
        if f in ("north", "south"):
            left, right = (x, x + 1) if f == "north" else (x + 1, x)
            c += [f"setblock {left} {y} {z0} chest[facing={f},type=left] strict",
                  f"setblock {right} {y} {z0} chest[facing={f},type=right] strict"]
        else:
            left, right = (z0, z0 + 1) if f == "east" else (z0 + 1, z0)
            c += [f"setblock {x} {y} {left} chest[facing={f},type=left] strict",
                  f"setblock {x} {y} {right} chest[facing={f},type=right] strict"]
    c += [f"setblock {x0 + 16} {y} {z0} trapped_chest[facing=south]", f"setblock {x0 + 18} {y} {z0} chest[facing=south]"]
    # beds, banners (standing, rotated; wall on a stone row), wool behind
    for i, d in enumerate(DYES):
        x = x0 + i * 2
        c += [f"setblock {x} {y} {z0 + 4} {d}_bed[facing=south,part=foot] strict",
              f"setblock {x} {y} {z0 + 5} {d}_bed[facing=south,part=head] strict",
              f"setblock {x} {y} {z0 + 8} {d}_banner[rotation={i}]",
              f"setblock {x} {y} {z0 + 10} stone", f"setblock {x} {y} {z0 + 11} {d}_wall_banner[facing=south]",
              f"setblock {x} {y} {z0 + 13} {d}_shulker_box[facing={['up', 'down', 'north', 'south', 'east', 'west'][i % 6]}]"]
    for i, plant in enumerate(["poppy", "cactus", "azalea_bush", "oak_sapling", "fern", "red_mushroom", "bamboo",
                               "dandelion", "blue_orchid", "wither_rose", "crimson_fungus", "dead_bush"]):
        c.append(f"setblock {x0 + i * 2} {y} {z0 + 15} potted_{plant}")
    for i in range(16):
        c.append(f"setblock {x0 + i * 2} {y} {z0 + 17} {['skeleton_skull', 'zombie_head', 'creeper_head', 'player_head'][i % 4]}[rotation={i}]")
    # redstone: a line, a corner, a step up, torch and repeater ends (each wire touches another, so Java's
    # updates reshape it as placing would)
    z = z0 + 20
    c += [f"setblock {x0 + i} {y} {z} redstone_wire" for i in range(5)]
    c += [f"setblock {x0 + 10} {y} {z} redstone_wire",
          f"setblock {x0 + 10} {y} {z + 1} redstone_wire", f"setblock {x0 + 11} {y} {z} redstone_wire",
          f"setblock {x0 + 13} {y} {z} stone", f"setblock {x0 + 13} {y + 1} {z} redstone_wire",
          f"setblock {x0 + 12} {y} {z} redstone_wire", f"setblock {x0 + 15} {y} {z} redstone_torch",
          f"setblock {x0 + 16} {y} {z} redstone_wire", f"setblock {x0 + 18} {y} {z} repeater[facing=west]",
          f"setblock {x0 + 17} {y} {z} redstone_wire", f"setblock {x0 + 20} {y} {z} lever[face=floor]",
          f"setblock {x0 + 21} {y} {z} redstone_wire"]
    # snowy grass, podzol, mycelium; grass without snow next to it
    z = z0 + 23
    for i, b in enumerate(["grass_block", "podzol", "mycelium"]):
        c += [f"setblock {x0 + i * 2} {y} {z} {b}", f"setblock {x0 + i * 2} {y + 1} {z} snow",
              f"setblock {x0 + i * 2 + 1} {y} {z} {b}"]
    c += [f"setblock {x0 + 8} {y} {z} grass_block", f"setblock {x0 + 8} {y + 1} {z} snow_block"]
    # a mushroom cap (3x3x2 with stem), a kelp tank, vines down from a beam
    z = z0 + 26
    c += [f"fill {x0} {y + 3} {z} {x0 + 2} {y + 4} {z + 2} red_mushroom_block",
          f"fill {x0 + 1} {y} {z + 1} {x0 + 1} {y + 2} {z + 1} mushroom_stem",
          f"fill {x0 + 6} {y} {z} {x0 + 8} {y + 5} {z + 2} glass",
          f"fill {x0 + 7} {y} {z + 1} {x0 + 7} {y + 4} {z + 1} water",
          f"setblock {x0 + 7} {y} {z + 1} sand",
          *[f"setblock {x0 + 7} {y + k} {z + 1} kelp" for k in (1, 2, 3)],
          f"fill {x0 + 11} {y + 5} {z} {x0 + 13} {y + 5} {z} stone",
          *[f"setblock {x0 + 12} {y + 5 - k} {z} weeping_vines" for k in (1, 2, 3)],
          f"setblock {x0 + 16} {y} {z} piston[facing=up,extended=true] strict",
          f"setblock {x0 + 16} {y + 1} {z} piston_head[facing=up] strict"]
    # stair corners from updates; fences, walls, panes in lines and a T
    z = z0 + 30
    c += [f"setblock {x0} {y} {z} oak_stairs[facing=north]", f"setblock {x0 + 1} {y} {z} oak_stairs[facing=west]",
          f"setblock {x0} {y} {z + 1} oak_stairs[facing=west]",
          *[f"setblock {x0 + 4 + i} {y} {z} oak_fence" for i in range(4)], f"setblock {x0 + 5} {y} {z + 1} oak_fence",
          *[f"setblock {x0 + 10 + i} {y} {z} cobblestone_wall" for i in range(4)],
          f"setblock {x0 + 11} {y} {z + 1} cobblestone_wall",
          *[f"setblock {x0 + 16 + i} {y} {z} glass_pane" for i in range(4)],
          *[f"setblock {x0 + 22 + i} {y} {z} iron_bars" for i in range(3)]]
    # a closed stone room with a torch, a room under a bottom-slab roof, a leaf canopy
    z = z0 + 34
    c += [f"fill {x0} {y} {z} {x0 + 6} {y + 4} {z + 6} stone hollow", f"setblock {x0 + 3} {y + 1} {z + 3} torch",
          f"fill {x0 + 10} {y} {z} {x0 + 16} {y + 3} {z + 6} oak_planks hollow",
          f"fill {x0 + 10} {y + 4} {z} {x0 + 16} {y + 4} {z + 6} oak_slab[type=bottom]",
          f"fill {x0 + 11} {y + 3} {z + 1} {x0 + 15} {y + 3} {z + 5} air",
          f"setblock {x0 + 10} {y + 1} {z + 3} air",
          f"fill {x0 + 20} {y + 4} {z} {x0 + 26} {y + 4} {z + 6} oak_leaves[persistent=true]",
          f"setblock {x0 + 23} {y} {z + 3} glowstone", f"setblock {x0 + 23} {y + 1} {z + 3} sea_lantern",
          f"setblock {x0 + 2} {y} {z + 9} lectern", f"setblock {x0 + 4} {y} {z + 9} campfire",
          f"setblock {x0 + 6} {y} {z + 9} jack_o_lantern", f"setblock {x0 + 8} {y} {z + 9} lantern"]
    return c
