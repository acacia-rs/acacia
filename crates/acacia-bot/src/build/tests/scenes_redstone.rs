//! The redstone tier's scenes, shared by the sim tests and `examples/build.rs` (`SCENE=rs-rails`, `rs-garden`),
//! which includes this file by path: the parent module brings `Block` and `Structure` into scope. Both start
//! with the floor (y 0) laid as [`FLOORS`] says: building it too left a floor cell beside a raised block whose
//! clicks only grazed the block's corner (sim: given up, "no stand"), a planner matter outside this tier.

use super::{Block, Structure};

fn put(s: &mut Structure, p: [i32; 3], name: &str, props: &str) {
    s.set(p, Some(Block::new(name, props)));
}

/// A stone floor with air above, `size` wide.
fn floor(size: [i32; 3], name: &str) -> Structure {
    let mut s = Structure::new(size);
    for x in 0..size[0] {
        for z in 0..size[2] {
            put(&mut s, [x, 0, z], name, "");
            (1..size[1]).for_each(|y| put(&mut s, [x, y, z], "air", ""));
        }
    }
    s
}

/// An east-west line ending in a curve south into a north-south leg, a stub joining the line's middle from the
/// north (a junction), a rising rail onto a block, powered and detector rails in a line, dust, a lava source.
pub fn rails() -> Structure {
    let mut s = floor([8, 3, 6], "stone");
    let rail = |s: &mut Structure, p: [i32; 3], name: &str, shape: u8| put(s, p, name, &format!("rail_direction={shape}"));
    (0..4).for_each(|x| rail(&mut s, [x, 1, 1], "rail", 1));
    rail(&mut s, [4, 1, 1], "rail", 7);
    rail(&mut s, [4, 1, 2], "rail", 0);
    rail(&mut s, [4, 1, 3], "rail", 0);
    rail(&mut s, [2, 1, 0], "rail", 0);
    rail(&mut s, [0, 1, 5], "rail", 1);
    rail(&mut s, [1, 1, 5], "rail", 2);
    put(&mut s, [2, 1, 5], "stone", "");
    rail(&mut s, [2, 2, 5], "rail", 1);
    for (x, name) in [(0, "golden_rail"), (1, "golden_rail"), (2, "detector_rail")] {
        put(&mut s, [x, 1, 3], name, "rail_data_bit=0,rail_direction=1");
    }
    (0..4).for_each(|z| put(&mut s, [6, 1, z], "redstone_wire", "redstone_signal=0"));
    put(&mut s, [5, 0, 4], "lava", "liquid_depth=0");
    s
}

/// Crops on tilled farmland by a water source, sugar cane on sand by water, cactus, a sapling, a flower, a vine
/// on a stone block, kelp and seagrass in sources.
pub fn garden() -> Structure {
    let mut s = floor([7, 3, 6], "grass_block");
    // Liquids only inside: a source on the edge would flow off the footprint.
    put(&mut s, [5, 0, 1], "water", "liquid_depth=0");
    for (x, crop) in [(1, "wheat"), (2, "carrots"), (3, "potatoes"), (4, "beetroot")] {
        put(&mut s, [x, 0, 1], "farmland", "moisturized_amount=0");
        put(&mut s, [x, 1, 1], crop, "growth=0");
    }
    put(&mut s, [0, 0, 3], "sand", "");
    put(&mut s, [1, 0, 3], "water", "liquid_depth=0");
    (1..3).for_each(|y| put(&mut s, [0, y, 3], "reeds", "age=0"));
    put(&mut s, [3, 0, 3], "sand", "");
    (1..3).for_each(|y| put(&mut s, [3, y, 3], "cactus", "age=0"));
    put(&mut s, [5, 0, 3], "kelp", "kelp_age=0");
    put(&mut s, [5, 0, 4], "seagrass", "sea_grass_type=default");
    put(&mut s, [5, 1, 5], "oak_sapling", "age_bit=0");
    put(&mut s, [3, 1, 5], "poppy", "");
    // The grass under it turns to dirt once it is placed: `rules::redstone::covered_grass`.
    put(&mut s, [0, 1, 5], "stone", "");
    put(&mut s, [1, 1, 5], "vine", "vine_direction_bits=2");
    s
}

/// The block each scene's floor is laid with before the build: (scene, block).
pub const FLOORS: [(&str, &str); 2] = [("rails", "stone"), ("garden", "grass_block")];

/// What the scenes use, for the stations: (item, count).
pub const STOCK: &[(&str, u32)] = &[
    ("stone", 64),
    ("rail", 16),
    ("golden_rail", 2),
    ("detector_rail", 1),
    ("redstone", 4),
    ("lava_bucket", 1),
    ("grass_block", 64),
    ("dirt", 8),
    ("water_bucket", 1),
    ("water_bucket", 1),
    ("water_bucket", 1),
    ("water_bucket", 1),
    ("wheat_seeds", 2),
    ("carrot", 2),
    ("potato", 2),
    ("beetroot_seeds", 2),
    ("sand", 4),
    ("sugar_cane", 4),
    ("cactus", 4),
    ("kelp", 2),
    ("seagrass", 2),
    ("oak_sapling", 2),
    ("poppy", 2),
    ("vine", 2),
];
