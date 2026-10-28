//! Live scenes for blocks one placement fills two cells (`build/rules/twocell.rs`), with a third barrel holding
//! their items: `doors` (a cobblestone wall with a double wooden door and an iron door, stone junk in the iron
//! door's upper cell), `beds` (two beds, facing east and north), `plants` (tall plants on grass, small dripleaf on
//! clay).

use acacia_bot::build::{Block, Structure};
use acacia_bot::Bot;

use super::{Error, Fill, Pad};

/// The third barrel (pad offset), beside the two `house` stations.
const STATION: [i32; 3] = [2, 0, -10];

fn put(s: &mut Structure, p: [i32; 3], name: &str, props: &str) {
    s.set(p, Some(Block::new(name, props)));
}

fn door(s: &mut Structure, p: [i32; 3], name: &str, hinge: u8) {
    let half = |u: u8, h: u8| format!("door_hinge_bit={h},minecraft:cardinal_direction=east,open_bit=0,upper_block_bit={u}");
    put(s, p, name, &half(0, 0));
    put(s, [p[0], p[1] + 1, p[2]], name, &half(1, hinge));
}

fn floor(size: [i32; 3], block: &str) -> Structure {
    let mut s = Structure::new(size);
    for x in 0..size[0] {
        for z in 0..size[2] {
            put(&mut s, [x, 0, z], block, "");
            (1..size[1]).for_each(|y| put(&mut s, [x, y, z], "air", ""));
        }
    }
    s
}

/// The scene's structure, junk (absolute pad offsets from `at`) and the third barrel's stock; `None`: not ours.
pub fn scene(name: &str, at: impl Fn([i32; 3]) -> [i32; 3]) -> Option<(Structure, Vec<Fill<'static>>, &'static [(&'static str, u32)])> {
    Some(match name {
        "doors" => {
            // Facing east: placed from the south looking north, west on the left. The right door of the pair has
            // a door on its left (hinge 1); the iron door has walls both sides (hinge 0).
            let mut s = floor([6, 3, 3], "oak_planks");
            (0..6).for_each(|x| (1..3).for_each(|y| put(&mut s, [x, y, 0], "cobblestone", "")));
            door(&mut s, [1, 1, 0], "wooden_door", 0);
            door(&mut s, [2, 1, 0], "wooden_door", 1);
            door(&mut s, [4, 1, 0], "iron_door", 0);
            (s, vec![(at([4, 2, 0]), at([4, 2, 0]), "stone")], &[("wooden_door", 4), ("iron_door", 2)])
        }
        "beds" => {
            let mut s = floor([5, 2, 4], "oak_planks");
            put(&mut s, [1, 1, 1], "bed", "direction=3,head_piece_bit=0,occupied_bit=0");
            put(&mut s, [2, 1, 1], "bed", "direction=3,head_piece_bit=1,occupied_bit=0");
            put(&mut s, [4, 1, 3], "bed", "direction=2,head_piece_bit=0,occupied_bit=0");
            put(&mut s, [4, 1, 2], "bed", "direction=2,head_piece_bit=1,occupied_bit=0");
            // Beds don't stack: a slot each.
            (s, vec![], &[("bed", 1), ("bed", 1), ("bed", 1)])
        }
        "plants" => {
            let mut s = floor([6, 3, 3], "grass_block");
            put(&mut s, [5, 0, 1], "clay", "");
            for (x, plant) in [(0, "sunflower"), (1, "tall_grass"), (2, "rose_bush"), (3, "pitcher_plant")] {
                put(&mut s, [x, 1, 1], plant, "upper_block_bit=0");
                put(&mut s, [x, 2, 1], plant, "upper_block_bit=1");
            }
            put(&mut s, [5, 1, 1], "small_dripleaf_block", "minecraft:cardinal_direction=south,upper_block_bit=0");
            put(&mut s, [5, 2, 1], "small_dripleaf_block", "minecraft:cardinal_direction=south,upper_block_bit=1");
            let stock: &[(&str, u32)] = &[("grass_block", 64), ("clay", 4), ("sunflower", 4), ("tall_grass", 4), ("rose_bush", 4), ("pitcher_plant", 4), ("small_dripleaf_block", 4)];
            (s, vec![], stock)
        }
        _ => return None,
    })
}

/// Places and fills the third barrel; returns its pad offset.
pub async fn station(a: &mut Bot, pad: &Pad, stock: &[(&str, u32)]) -> Result<[i32; 3], Error> {
    let [x, y, z] = pad.block(STATION);
    pad.run(a, &format!("/setblock {x} {y} {z} barrel"));
    a.wait_ticks(2).await?;
    for (slot, (item, count)) in stock.iter().enumerate() {
        pad.run(a, &format!("/replaceitem block {x} {y} {z} slot.container {slot} {item} {count}"));
        a.wait_ticks(2).await?;
    }
    Ok(STATION)
}
