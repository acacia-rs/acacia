//! The interact tier's live scenes (`examples/build.rs SCENE=`): `interact-use` (use-toggled states, two of them
//! standing with the wrong toggle), `interact-count` (candles, sea pickles, turtle eggs), `interact-sign` (standing,
//! wall and hanging signs with text on both sides).

use acacia_bot::build::{Block, SignText, Structure};
use acacia_bot::Bot;

use super::{Error, Fill, Pad};

/// Station slots past the house stock, as (station, slot, item).
const STOCK: &[(usize, u8, &str)] = &[
    (0, 23, "repeater"), (0, 24, "comparator"), (0, 25, "lever"), (0, 26, "daylight_detector"), (1, 19, "white_candle"), (1, 20, "sea_pickle"),
    (1, 21, "turtle_egg"), (1, 22, "oak_sign"), (1, 23, "oak_hanging_sign"), (1, 24, "blue_candle"),
];

fn floor(size: [i32; 3]) -> Structure {
    let mut s = Structure::new(size);
    for x in 0..size[0] {
        for z in 0..size[2] {
            s.set([x, 0, z], Some(Block::new("oak_planks", "")));
        }
    }
    s
}

/// The scene's structure and junk (pad offsets, `at` the footprint corner); `None`: not one of these.
pub fn scene(name: &str, at: [i32; 3]) -> Option<(Structure, Vec<Fill<'static>>)> {
    let put = |s: &mut Structure, p: [i32; 3], name: &str, props: &str| s.set(p, Some(Block::new(name, props)));
    let o = |p: [i32; 3]| [at[0] + p[0], at[1] + p[1], at[2] + p[2]];
    Some(match name {
        "interact-use" => {
            let mut s = floor([7, 2, 3]);
            put(&mut s, [0, 1, 1], "unpowered_repeater", "minecraft:cardinal_direction=east,repeater_delay=2");
            put(&mut s, [2, 1, 1], "unpowered_comparator", "minecraft:cardinal_direction=north,output_lit_bit=0,output_subtract_bit=1");
            put(&mut s, [4, 1, 1], "lever", "lever_direction=up_north_south,open_bit=1");
            put(&mut s, [6, 1, 1], "daylight_detector_inverted", "redstone_signal=0");
            put(&mut s, [3, 1, 2], "lever", "lever_direction=east,open_bit=0");
            put(&mut s, [2, 1, 2], "cobblestone", "");
            let junk = vec![
                (o([4, 0, 1]), o([4, 0, 1]), "oak_planks"),
                (o([4, 1, 1]), o([4, 1, 1]), "lever [\"lever_direction\"=\"up_north_south\",\"open_bit\"=false]"),
                (o([0, 0, 1]), o([0, 0, 1]), "oak_planks"),
                (o([0, 1, 1]), o([0, 1, 1]), "unpowered_repeater [\"minecraft:cardinal_direction\"=\"east\",\"repeater_delay\"=3]"),
            ];
            (s, junk)
        }
        "interact-count" => {
            let mut s = floor([7, 2, 3]);
            put(&mut s, [0, 1, 1], "white_candle", "candles=3,lit=0");
            put(&mut s, [2, 1, 1], "blue_candle", "candles=1,lit=0");
            put(&mut s, [4, 1, 1], "sea_pickle", "cluster_count=2,dead_bit=1");
            put(&mut s, [6, 1, 1], "turtle_egg", "cracked_state=no_cracks,turtle_egg_count=three_egg");
            (s, Vec::new())
        }
        "interact-sign" => {
            let mut s = floor([7, 2, 4]);
            let mut sign = |s: &mut Structure, p: [i32; 3], name: &str, props: &str, front: &str, back: &str| {
                put(s, p, name, props);
                s.set_text(p, Some(SignText { front: front.into(), back: back.into() }));
            };
            sign(&mut s, [1, 1, 2], "standing_sign", "ground_sign_direction=6", "standing\nsix", "its back");
            sign(&mut s, [4, 1, 3], "standing_sign", "ground_sign_direction=0", "south", "");
            put(&mut s, [3, 1, 0], "cobblestone", "");
            sign(&mut s, [3, 1, 1], "wall_sign", "facing_direction=3", "on the wall", "");
            put(&mut s, [5, 1, 0], "cobblestone", "");
            sign(&mut s, [6, 1, 0], "oak_hanging_sign", "attached_bit=0,facing_direction=3,ground_sign_direction=0,hanging=0", "hanging", "behind");
            (s, Vec::new())
        }
        _ => return None,
    })
}

/// Puts this tier's items into the stations (house16's stock leaves these slots free).
pub async fn stock(a: &mut Bot, pad: &Pad, stations: &[[i32; 3]]) -> Result<(), Error> {
    for (station, slot, item) in STOCK {
        let [x, y, z] = pad.block(stations[*station]);
        pad.run(a, &format!("/replaceitem block {x} {y} {z} slot.container {slot} {item} 16"));
        a.wait_ticks(2).await?;
    }
    Ok(())
}
