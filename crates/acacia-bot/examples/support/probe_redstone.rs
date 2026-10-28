//! `SET=redstone` for `place_probe`: rails in placement orders, plants on their supports, buckets and a hoe.
//! Each case clears a box over the pad floor, sets its blocks, places or uses items in order and prints the
//! cells it watches after each step. `ONLY=<case>`.

use acacia_bot::interact::Face;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;

use crate::support::{Error, Pad};

/// Centre cell of every case (pad offset): on the floor, which is y -1.
const C: [i32; 3] = [5, 0, 0];
const N: [i32; 3] = [5, 0, -1];
const S: [i32; 3] = [5, 0, 1];
const E: [i32; 3] = [6, 0, 0];
const W: [i32; 3] = [4, 0, 0];
const UP_E: [i32; 3] = [6, 1, 0];
const BELOW: [i32; 3] = [5, -1, 0];

/// Feet stands: north and west of C on the floor, and on a stone ring round C (water cases).
const FROM_N: [f32; 3] = [5.5, 0.0, -2.5];
const FROM_W: [f32; 3] = [2.5, 0.0, 0.5];
const ON_RING: [f32; 3] = [5.5, 1.0, -0.5];
/// On a two-high ring (water two deep at C).
const ON_RING2: [f32; 3] = [5.5, 2.0, -0.5];

enum Step {
    /// Place `item` into the cell: clicking `face` of the block the face's offset back from it.
    Put(&'static str, [i32; 3], Face),
    /// [`Step::Put`] clicking the Up face at this height (farmland's top is 15/16).
    PutAt(&'static str, [i32; 3], f32),
    /// Use `item` on `face` of the block at the offset.
    Use(&'static str, [i32; 3], Face),
}

struct Case {
    name: &'static str,
    stand: [f32; 3],
    blocks: &'static [([i32; 3], &'static str)],
    steps: &'static [Step],
    watch: &'static [[i32; 3]],
}

const RING: [([i32; 3], &str); 8] = [
    ([4, 0, -1], "stone"),
    ([5, 0, -1], "stone"),
    ([6, 0, -1], "stone"),
    ([4, 0, 0], "stone"),
    ([6, 0, 0], "stone"),
    ([4, 0, 1], "stone"),
    ([5, 0, 1], "stone"),
    ([6, 0, 1], "stone"),
];

use Step::{Put, PutAt, Use};

const RAILS: &[[i32; 3]] = &[C, N, S, E, W, UP_E, [4, 0, -1], [6, 0, -1], [4, 0, 1], [6, 0, 1]];

macro_rules! rails {
    ($name:expr, $item:expr, $stand:expr, [$($c:expr),*]) => {
        Case { name: $name, stand: $stand, blocks: &[], steps: &[$(Put($item, $c, Face::Up)),*], watch: RAILS }
    };
}

const CASES: &[Case] = &[
    rails!("rail-lone-n", "rail", FROM_N, [C]),
    rails!("rail-lone-w", "rail", FROM_W, [C]),
    rails!("rail-pair-e", "rail", FROM_N, [C, E]),
    rails!("rail-pair-n", "rail", FROM_W, [C, N]),
    rails!("rail-mid-last", "rail", FROM_N, [W, E, C]),
    rails!("rail-curve-last", "rail", FROM_N, [N, E, C]),
    rails!("rail-curve-second", "rail", FROM_N, [C, N, E]),
    rails!("rail-curve-sw", "rail", FROM_N, [S, W, C]),
    rails!("rail-tee-full", "rail", FROM_N, [W, C, E, N]),
    rails!("rail-tee-open", "rail", FROM_N, [W, C, N, E]),
    rails!("rail-three-new", "rail", FROM_N, [N, E, W, C]),
    rails!("rail-three-new2", "rail", FROM_N, [E, S, W, C]),
    rails!("rail-three-new3", "rail", FROM_N, [N, S, E, C]),
    rails!("rail-four-new", "rail", FROM_N, [N, S, E, W, C]),
    rails!("rail-steal", "rail", FROM_N, [W, C, [6, 0, -1], E]),
    Case { name: "rail-ascend-up-last", stand: FROM_N, blocks: &[(E, "stone")], steps: &[Put("rail", C, Face::Up), Put("rail", UP_E, Face::Up)], watch: RAILS },
    Case { name: "rail-ascend-low-last", stand: FROM_N, blocks: &[(E, "stone")], steps: &[Put("rail", UP_E, Face::Up), Put("rail", C, Face::Up)], watch: RAILS },
    rails!("golden-curve-last", "golden_rail", FROM_N, [N, E, C]),
    rails!("golden-pair-e", "golden_rail", FROM_N, [C, E]),
    rails!("golden-lone-w", "golden_rail", FROM_W, [C]),
    rails!("detector-pair-e", "detector_rail", FROM_N, [C, E]),
    rails!("activator-curve-second", "activator_rail", FROM_N, [C, N, E]),
    Case { name: "mixed-rail-golden", stand: FROM_N, blocks: &[], steps: &[Put("golden_rail", N, Face::Up), Put("rail", E, Face::Up), Put("rail", C, Face::Up)], watch: RAILS },
    Case { name: "redstone", stand: FROM_N, blocks: &[], steps: &[Put("redstone", C, Face::Up), Put("redstone", E, Face::Up)], watch: &[C, E] },
    Case { name: "redstone-side", stand: FROM_N, blocks: &[(W, "stone")], steps: &[Put("redstone", C, Face::East)], watch: &[C] },
    Case { name: "sapling-grass", stand: FROM_N, blocks: &[(BELOW, "grass_block")], steps: &[Put("oak_sapling", C, Face::Up)], watch: &[C] },
    Case { name: "sapling-dirt-side", stand: FROM_N, blocks: &[(BELOW, "dirt"), (E, "stone")], steps: &[Put("oak_sapling", C, Face::West)], watch: &[C] },
    Case { name: "sapling-stone", stand: FROM_N, blocks: &[], steps: &[Put("birch_sapling", C, Face::Up)], watch: &[C] },
    Case { name: "poppy-grass", stand: FROM_N, blocks: &[(BELOW, "grass_block")], steps: &[Put("poppy", C, Face::Up), Put("dandelion", W, Face::Up)], watch: &[C, W] },
    Case { name: "mushroom", stand: FROM_N, blocks: &[(BELOW, "dirt")], steps: &[Put("brown_mushroom", C, Face::Up), Put("red_mushroom", E, Face::Up)], watch: &[C, E] },
    Case { name: "cane-water", stand: FROM_N, blocks: &[(BELOW, "sand"), ([6, -1, 0], "water")], steps: &[Put("sugar_cane", C, Face::Up), Put("sugar_cane", [5, 1, 0], Face::Up)], watch: &[C, [5, 1, 0]] },
    Case { name: "cane-dry", stand: FROM_N, blocks: &[(BELOW, "sand")], steps: &[Put("sugar_cane", C, Face::Up)], watch: &[C] },
    Case { name: "cactus", stand: FROM_N, blocks: &[(BELOW, "sand")], steps: &[Put("cactus", C, Face::Up), Put("cactus", [5, 1, 0], Face::Up)], watch: &[C, [5, 1, 0]] },
    Case { name: "cactus-beside", stand: FROM_N, blocks: &[(BELOW, "sand"), (E, "stone")], steps: &[Put("cactus", C, Face::Up)], watch: &[C] },
    Case { name: "bamboo", stand: FROM_N, blocks: &[(BELOW, "dirt")], steps: &[Put("bamboo", C, Face::Up), Put("bamboo", [5, 1, 0], Face::Up), Put("bamboo", [5, 2, 0], Face::Up)], watch: &[C, [5, 1, 0], [5, 2, 0]] },
    Case { name: "vine-side", stand: FROM_N, blocks: &[(E, "stone")], steps: &[Put("vine", C, Face::West)], watch: &[C] },
    Case { name: "vine-two-sides", stand: FROM_N, blocks: &[(E, "stone"), (W, "stone"), ([5, 1, 0], "stone")], steps: &[Put("vine", C, Face::West)], watch: &[C] },
    Case { name: "vine-ceiling", stand: FROM_N, blocks: &[([5, 1, 0], "stone")], steps: &[Put("vine", C, Face::Down)], watch: &[C] },
    Case { name: "vine-floor", stand: FROM_N, blocks: &[], steps: &[Put("vine", C, Face::Up)], watch: &[C] },
    Case { name: "kelp", stand: ON_RING, blocks: &[(C, "water")], steps: &[Put("kelp", C, Face::Up)], watch: &[C] },
    Case { name: "seagrass", stand: ON_RING, blocks: &[(C, "water")], steps: &[Put("seagrass", C, Face::Up)], watch: &[C] },
    Case { name: "kelp-dry", stand: FROM_N, blocks: &[], steps: &[Put("kelp", C, Face::Up)], watch: &[C] },
    Case { name: "water-bucket", stand: ON_RING, blocks: &[], steps: &[Use("water_bucket", BELOW, Face::Up)], watch: &[C] },
    Case { name: "water-bucket-side", stand: ON_RING, blocks: &[], steps: &[Use("water_bucket", W, Face::East)], watch: &[C] },
    Case { name: "lava-bucket", stand: ON_RING, blocks: &[], steps: &[Use("lava_bucket", BELOW, Face::Up)], watch: &[C] },
    Case { name: "till-dirt", stand: FROM_N, blocks: &[(BELOW, "dirt")], steps: &[Use("wooden_hoe", BELOW, Face::Up), PutAt("wheat_seeds", C, 0.9375)], watch: &[BELOW, C] },
    Case { name: "till-grass", stand: FROM_N, blocks: &[(BELOW, "grass_block")], steps: &[Use("wooden_hoe", BELOW, Face::Up), PutAt("carrot", C, 0.9375)], watch: &[BELOW, C] },
    Case { name: "kelp-stack", stand: ON_RING, blocks: &[(C, "water")], steps: &[Put("kelp", C, Face::Up), Put("kelp", [5, 1, 0], Face::Up)], watch: &[C, [5, 1, 0]] },
    Case { name: "seagrass-stack", stand: ON_RING, blocks: &[(C, "water")], steps: &[Put("seagrass", C, Face::Up), Put("seagrass", [5, 1, 0], Face::Up)], watch: &[C, [5, 1, 0]] },
    Case { name: "till-side", stand: FROM_N, blocks: &[([5, 0, -1], "dirt")], steps: &[Use("wooden_hoe", [5, 0, -1], Face::North)], watch: &[[5, 0, -1]] },
    Case { name: "till-covered", stand: FROM_N, blocks: &[([5, 0, -1], "dirt"), ([5, 1, -1], "stone")], steps: &[Use("wooden_hoe", [5, 0, -1], Face::North)], watch: &[[5, 0, -1]] },
    Case { name: "crops", stand: FROM_N, blocks: &[(BELOW, "farmland"), ([4, -1, 0], "farmland"), ([6, -1, 0], "farmland")], steps: &[PutAt("potato", C, 0.9375), PutAt("beetroot_seeds", W, 0.9375), PutAt("wheat_seeds", E, 0.9375)], watch: &[C, W, E, BELOW] },
    Case { name: "seeds-dirt", stand: FROM_N, blocks: &[(BELOW, "dirt")], steps: &[Put("wheat_seeds", C, Face::Up)], watch: &[C] },
];

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub async fn run(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    let only = std::env::var("ONLY").ok();
    for case in CASES.iter().filter(|c| only.as_deref().is_none_or(|o| o.split(',').any(|o| c.name.starts_with(o)))) {
        reset(bot, pad, case).await?;
        let yaw = {
            let (dx, dz) = (C[0] as f32 + 0.5 - case.stand[0], C[2] as f32 + 0.5 - case.stand[2]);
            (-dx).atan2(dz).to_degrees()
        };
        pad.teleport(bot, case.stand, yaw).await?;
        for (i, step) in case.steps.iter().enumerate() {
            let r = match step {
                Put(item, cell, face) => {
                    let o = face.offset();
                    put(bot, pad, add(*cell, [-o[0], -o[1], -o[2]]), *face, face.click_offset(), item).await
                }
                PutAt(item, cell, y) => put(bot, pad, add(*cell, [0, -1, 0]), Face::Up, [0.5, *y, 0.5], item).await,
                Use(item, at, face) => use_on(bot, pad, *at, *face, item).await,
            };
            let held = bot.state().items.name(bot.state().inventory.held().network_id).unwrap_or("-").to_string();
            let cells: Vec<String> = case.watch.iter().filter_map(|c| bot.block_state(pad.block(*c)).filter(|s| !s.is_air()).map(|s| format!("{c:?}={} [{}]", s.name.trim_start_matches("minecraft:"), s.properties))).collect();
            println!("{:<24} step {i} {r} held {held}: {}", case.name, cells.join(" "));
        }
    }
    Ok(())
}

async fn reset(bot: &mut Bot, pad: &Pad, case: &Case) -> Result<(), Error> {
    let (lo, hi, flo, fhi) = (pad.block([1, 0, -4]), pad.block([9, 4, 4]), pad.block([1, -1, -4]), pad.block([9, -1, 4]));
    let mut cmds = vec![format!("/fill {} {} {} {} {} {} air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]), format!("/fill {} {} {} {} {} {} stone", flo[0], flo[1], flo[2], fhi[0], fhi[1], fhi[2])];
    let rings = if case.stand == ON_RING { 1 } else if case.stand == ON_RING2 { 2 } else { 0 };
    let ring = (0..rings).flat_map(|y| RING.iter().map(move |(c, b)| ([c[0], y, c[2]], *b)));
    let extra = ring.chain(case.blocks.iter().copied());
    cmds.extend(extra.map(|(c, b)| {
        let p = pad.block(c);
        format!("/setblock {} {} {} {b}", p[0], p[1], p[2])
    }));
    for c in ["/clear @s".to_string(), "/time set noon".to_string()].into_iter().chain(cmds) {
        pad.run(bot, &c);
        bot.wait_ticks(2).await?;
    }
    for item in case.steps.iter().map(|s| match s {
        Put(i, ..) | PutAt(i, ..) | Use(i, ..) => *i,
    }) {
        pad.run(bot, &format!("/give @s {item} 4"));
        bot.wait_ticks(2).await?;
    }
    Ok(bot.wait_ticks(10).await?)
}

async fn use_on(bot: &mut Bot, pad: &Pad, at: [i32; 3], face: Face, item: &str) -> String {
    let Some(slot) = bot.find_item(&format!("minecraft:{item}")) else { return "no item".into() };
    if let Err(e) = bot.equip(slot, Destination::Hand).await {
        return format!("equip: {e}");
    }
    let r = bot.use_item_on_block(pad.block(at), face).await;
    let _ = bot.wait_ticks(10).await;
    match r {
        Ok(()) => "used".into(),
        Err(e) => format!("ERR {e}"),
    }
}

async fn put(bot: &mut Bot, pad: &Pad, against: [i32; 3], face: Face, click: [f32; 3], item: &str) -> String {
    let Some(slot) = bot.find_item(&format!("minecraft:{item}")) else { return "no item".into() };
    if let Err(e) = bot.equip(slot, Destination::Hand).await {
        return format!("equip: {e}");
    }
    let r = bot.place_block_at(pad.block(against), face, click).await;
    let _ = bot.wait_ticks(4).await;
    match r {
        Ok(()) => "placed".into(),
        Err(e) => format!("ERR {e}"),
    }
}
