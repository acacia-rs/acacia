//! The oneclick tier's live scenes (docs/builder/oneclick.md), each a few families on a small plank floor:
//! `gates` (trapdoors both halves, fence gates), `lights` (a hanging lantern and a chandelier under a ceiling, a
//! standing lantern, rods, a ceiling button), `stations` (a chest pair, barrel, hopper, anvil, bell, grindstone, loom, plate,
//! carpet, a lit furnace), `redstone` (block, lamp and torch lit and unlit by it, leaves, hay).

use acacia_bot::build::{Block, Structure};

/// What the first barrel holds for `scene`, as (item, stacks); `None`: not a oneclick scene.
pub fn stock(scene: &str) -> Option<&'static [(&'static str, u32)]> {
    Some(match scene {
        "gates" => &[("oak_planks", 1), ("cobblestone", 1), ("trapdoor", 1), ("iron_trapdoor", 1), ("fence_gate", 1), ("spruce_fence_gate", 1)],
        "lights" => &[("oak_planks", 1), ("cobblestone", 1), ("lantern", 1), ("soul_lantern", 1), ("iron_chain", 1), ("end_rod", 1), ("lightning_rod", 1), ("stone_button", 1)],
        "stations" => &[
            ("oak_planks", 1), ("cobblestone", 1), ("chest", 1), ("barrel", 1), ("hopper", 1), ("anvil", 1), ("bell", 1), ("grindstone", 1), ("loom", 1), ("furnace", 1),
            ("stone_pressure_plate", 1), ("white_carpet", 1),
        ],
        "redstone" => &[("oak_planks", 1), ("redstone_block", 1), ("redstone_lamp", 1), ("redstone_torch", 1), ("oak_leaves", 1), ("hay_block", 1)],
        _ => return None,
    })
}

fn floor(size: [i32; 3]) -> Structure {
    let mut s = Structure::new(size);
    for x in 0..size[0] {
        for z in 0..size[2] {
            s.set([x, 0, z], Some(Block::new("oak_planks", "")));
            (1..size[1]).for_each(|y| s.set([x, y, z], Some(Block::new("air", ""))));
        }
    }
    s
}

pub fn scene(name: &str) -> Option<Structure> {
    let put = |s: &mut Structure, p: [i32; 3], name: &str, props: &str| s.set(p, Some(Block::new(name, props)));
    let s = match name {
        "gates" => {
            let mut s = floor([5, 3, 3]);
            (0..5).for_each(|x| (1..3).for_each(|y| put(&mut s, [x, y, 0], "cobblestone", "")));
            put(&mut s, [1, 1, 1], "trapdoor", "direction=2,open_bit=0,upside_down_bit=0");
            put(&mut s, [3, 2, 1], "trapdoor", "direction=2,open_bit=0,upside_down_bit=1");
            put(&mut s, [2, 1, 2], "iron_trapdoor", "direction=0,open_bit=0,upside_down_bit=0");
            put(&mut s, [0, 1, 2], "fence_gate", "in_wall_bit=0,minecraft:cardinal_direction=east,open_bit=0");
            put(&mut s, [4, 1, 2], "spruce_fence_gate", "in_wall_bit=0,minecraft:cardinal_direction=north,open_bit=0");
            s
        }
        "lights" => {
            let mut s = floor([5, 5, 3]);
            for y in 1..5 {
                put(&mut s, [0, y, 1], "cobblestone", "");
                put(&mut s, [4, y, 1], "cobblestone", "");
            }
            (1..4).for_each(|x| put(&mut s, [x, 4, 1], "cobblestone", ""));
            put(&mut s, [1, 3, 1], "lantern", "hanging=1");
            put(&mut s, [3, 3, 1], "iron_chain", "pillar_axis=y");
            put(&mut s, [3, 2, 1], "soul_lantern", "hanging=1");
            put(&mut s, [2, 3, 1], "stone_button", "button_pressed_bit=0,facing_direction=0");
            put(&mut s, [1, 1, 0], "soul_lantern", "hanging=0");
            put(&mut s, [2, 1, 2], "end_rod", "facing_direction=1");
            put(&mut s, [3, 1, 2], "lightning_rod", "facing_direction=1,powered_bit=0");
            s
        }
        "stations" => {
            let mut s = floor([6, 2, 3]);
            put(&mut s, [0, 1, 0], "chest", "minecraft:cardinal_direction=south");
            put(&mut s, [1, 1, 0], "chest", "minecraft:cardinal_direction=south");
            put(&mut s, [3, 1, 0], "barrel", "facing_direction=3,open_bit=0");
            put(&mut s, [4, 1, 0], "lit_furnace", "minecraft:cardinal_direction=south");
            put(&mut s, [5, 1, 0], "hopper", "facing_direction=0,toggle_bit=0");
            put(&mut s, [0, 1, 2], "anvil", "minecraft:cardinal_direction=east");
            put(&mut s, [2, 1, 2], "bell", "attachment=standing,direction=0,toggle_bit=0");
            put(&mut s, [3, 1, 2], "grindstone", "attachment=standing,direction=1");
            put(&mut s, [5, 1, 2], "loom", "direction=2");
            put(&mut s, [2, 1, 1], "stone_pressure_plate", "redstone_signal=0");
            put(&mut s, [4, 1, 1], "white_carpet", "");
            s
        }
        "redstone" => {
            let mut s = floor([5, 3, 3]);
            put(&mut s, [1, 1, 1], "redstone_block", "");
            put(&mut s, [2, 1, 1], "lit_redstone_lamp", "");
            put(&mut s, [1, 2, 1], "unlit_redstone_torch", "torch_facing_direction=top");
            put(&mut s, [3, 1, 2], "redstone_torch", "torch_facing_direction=top");
            put(&mut s, [4, 1, 0], "redstone_lamp", "");
            put(&mut s, [0, 1, 0], "oak_leaves", "persistent_bit=0,update_bit=0");
            put(&mut s, [3, 1, 0], "hay_block", "deprecated=0,pillar_axis=y");
            s
        }
        _ => return None,
    };
    Some(s)
}
