//! `SET=twocell`: doors, beds and tall plants, one placement filling two cells. Each case resets an open floor
//! around T, adds its neighbours, places from a stand with a chosen click and prints both cells (and the bed's
//! block entity). Evidence for `build/rules/twocell.rs`.

use acacia_bot::interact::Face;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;

use super::support::{Error, Pad};

/// (give arguments, item to hold, support block under T).
const ITEMS: &[(&str, &str, &str)] = &[
    ("wooden_door", "wooden_door", "stone"),
    ("iron_door", "iron_door", "stone"),
    ("spruce_door", "spruce_door", "stone"),
    ("copper_door", "copper_door", "stone"),
    ("waxed_oxidized_copper_door", "waxed_oxidized_copper_door", "stone"),
    ("bed 64 0", "bed", "stone"),
    ("bed 64 14", "bed", "stone"),
    ("red_bed 64", "red_bed", "stone"),
    ("sunflower", "sunflower", "grass_block"),
    ("lilac", "lilac", "grass_block"),
    ("rose_bush", "rose_bush", "grass_block"),
    ("peony", "peony", "dirt"),
    ("tall_grass", "tall_grass", "grass_block"),
    ("large_fern", "large_fern", "dirt"),
    ("double_plant 64 0", "double_plant", "grass_block"),
    ("pitcher_plant", "pitcher_plant", "grass_block"),
    ("small_dripleaf_block", "small_dripleaf_block", "grass_block"),
    ("small_dripleaf_block", "small_dripleaf_block", "clay"),
    ("small_dripleaf_block", "small_dripleaf_block", "moss_block"),
];

/// The target cell on the pad floor (pad offset).
const T: [i32; 3] = [5, 0, 0];

/// Stands two blocks off T: (name, feet).
const STANDS: &[(&str, [f32; 3])] = &[("N", [5.5, 0.0, -1.5]), ("S", [5.5, 0.0, 2.5]), ("W", [3.5, 0.0, 0.5]), ("E", [7.5, 0.0, 0.5])];

/// Up-face click points on the support (x, z).
const POINTS: &[[f32; 2]] = &[[0.5, 0.5], [0.2, 0.2], [0.8, 0.2], [0.2, 0.8], [0.8, 0.8]];

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Door neighbours, stood south of T (looking north): (name, setblocks as (offset from T, block)).
fn door_cases(door: &str) -> Vec<(&'static str, Vec<([i32; 3], String)>)> {
    // Facing as a door placed from the south gets (see the plain cases).
    let d = |hinge: u8| format!("{door} [\"minecraft:cardinal_direction\"=\"east\",\"door_hinge_bit\"={},\"upper_block_bit\"=false]", hinge == 1);
    let u = |hinge: u8| format!("{door} [\"minecraft:cardinal_direction\"=\"east\",\"door_hinge_bit\"={},\"upper_block_bit\"=true]", hinge == 1);
    let alt = if door.contains("iron") { "minecraft:wooden_door" } else { "minecraft:iron_door" };
    let other = |up: bool| format!("{alt} [\"minecraft:cardinal_direction\"=\"east\",\"upper_block_bit\"={up}]");
    vec![
        ("stone-west", vec![([-1, 0, 0], "stone".into()), ([-1, 1, 0], "stone".into())]),
        ("stone-east", vec![([1, 0, 0], "stone".into()), ([1, 1, 0], "stone".into())]),
        ("stone-west-low", vec![([-1, 0, 0], "stone".into())]),
        ("stone-west-high", vec![([-1, 1, 0], "stone".into())]),
        ("stone-both", vec![([-1, 0, 0], "stone".into()), ([-1, 1, 0], "stone".into()), ([1, 0, 0], "stone".into()), ([1, 1, 0], "stone".into())]),
        ("door-west-h0", vec![([-1, 0, 0], d(0)), ([-1, 1, 0], u(0))]),
        ("door-east-h0", vec![([1, 0, 0], d(0)), ([1, 1, 0], u(0))]),
        ("door-west-h1", vec![([-1, 0, 0], d(1)), ([-1, 1, 0], u(1))]),
        ("door-east-h1", vec![([1, 0, 0], d(1)), ([1, 1, 0], u(1))]),
        ("upper-blocked", vec![([0, 1, 0], "stone".into())]),
        ("stone-east-low", vec![([1, 0, 0], "stone".into())]),
        ("stone-east-high", vec![([1, 1, 0], "stone".into())]),
        ("stone-west-low-east-both", vec![([-1, 0, 0], "stone".into()), ([1, 0, 0], "stone".into()), ([1, 1, 0], "stone".into())]),
        ("door-west-stone-east", vec![([-1, 0, 0], d(0)), ([-1, 1, 0], u(0)), ([1, 0, 0], "stone".into()), ([1, 1, 0], "stone".into())]),
        ("door-west-lower-only", vec![([-1, 0, 0], d(0))]),
        ("door-west-facing-west", vec![([-1, 0, 0], d(0).replace("east", "west")), ([-1, 1, 0], u(0).replace("east", "west"))]),
        ("door-west-facing-north", vec![([-1, 0, 0], d(0).replace("east", "north")), ([-1, 1, 0], u(0).replace("east", "north"))]),
        ("other-door-west", vec![([-1, 0, 0], other(false)), ([-1, 1, 0], other(true))]),
        ("door-both", vec![([-1, 0, 0], d(0)), ([-1, 1, 0], u(0)), ([1, 0, 0], d(0)), ([1, 1, 0], u(0))]),
        ("glass-east", vec![([1, 0, 0], "glass".into()), ([1, 1, 0], "glass".into())]),
        ("slab-east", vec![([1, 0, 0], "oak_slab".into()), ([1, 1, 0], "oak_slab".into())]),
        ("fence-east", vec![([1, 0, 0], "oak_fence".into()), ([1, 1, 0], "oak_fence".into())]),
    ]
}

pub async fn run(bot: &mut Bot, pad: &Pad, only: Option<&str>) -> Result<(), Error> {
    for &(give, hold, support) in ITEMS.iter().filter(|(g, _, _)| only.is_none_or(|o| g.starts_with(o))) {
        let count = if give.contains(' ') { "" } else { " 64" };
        for c in ["/clear @s".to_string(), format!("/give @s {give}{count}")] {
            pad.run(bot, &c);
            bot.wait_ticks(4).await?;
        }
        let label = format!("{give}/{support}");
        // HINGE=1: only the door neighbour cases, clicked at the centre, from the south and once from the north.
        let hinge = std::env::var("HINGE").is_ok();
        if hinge {
            let extra = [([-1, 0, 0], "stone".to_string()), ([-1, 1, 0], "stone".to_string())];
            let r = case(bot, pad, hold, support, &extra, STANDS[0].1, [0, -1, 0], Face::Up, [0.5, 1.0, 0.5]).await?;
            println!("{label:<34} N stone-west Up@0.5 -> {r}");
        }
        for &(stand, feet) in STANDS.iter().filter(|_| !hinge) {
            for p in POINTS {
                let r = case(bot, pad, hold, support, &[], feet, [0, -1, 0], Face::Up, [p[0], 1.0, p[1]]).await?;
                println!("{label:<34} {stand} Up@{:.1},{:.1} -> {r}", p[0], p[1]);
                if !hold.ends_with("door") {
                    break;
                }
            }
            // A side click into T: on the far neighbour's face towards T, low and high.
            let far = match stand {
                "N" => ([0, 0, 1], Face::North, [0.5, 0.0, 0.0]),
                "S" => ([0, 0, -1], Face::South, [0.5, 0.0, 1.0]),
                "W" => ([1, 0, 0], Face::West, [0.0, 0.0, 0.5]),
                _ => ([-1, 0, 0], Face::East, [1.0, 0.0, 0.5]),
            };
            for y in [0.25, 0.75] {
                let (off, face, mut click) = far;
                click[1] = y;
                let r = case(bot, pad, hold, support, &[(off, "stone".into())], feet, off, face, click).await?;
                println!("{label:<34} {stand} {face:?}@{y:.2} -> {r}");
            }
        }
        if hold.ends_with("door") {
            let feet = STANDS[1].1;
            for (name, extra) in door_cases(&format!("minecraft:{hold}")) {
                for p in [[0.5, 0.5], [0.2, 0.5], [0.8, 0.5]].into_iter().take(if hinge { 1 } else { 3 }) {
                    let r = case(bot, pad, hold, support, &extra, feet, [0, -1, 0], Face::Up, [p[0], 1.0, p[1]]).await?;
                    println!("{label:<34} S {name} Up@{:.1} -> {r}", p[0]);
                }
            }
        }
        if !hold.ends_with("door") {
            let r = case(bot, pad, hold, support, &[([0, 1, 0], "stone".into())], STANDS[1].1, [0, -1, 0], Face::Up, [0.5, 1.0, 0.5]).await?;
            println!("{label:<34} S upper-blocked -> {r}");
            // Bed foot cell open, head cell blocked.
            let r = case(bot, pad, hold, support, &[([0, 0, -1], "stone".into())], STANDS[1].1, [0, -1, 0], Face::Up, [0.5, 1.0, 0.5]).await?;
            println!("{label:<34} S ahead-blocked -> {r}");
        }
        if hold == "bed" {
            // A bed set by command (the junk of a live scene): which colour its block entity gets.
            let set = vec![([0, 0, 0], "bed [\"direction\"=2,\"head_piece_bit\"=false]".to_string()), ([0, 0, -1], "bed [\"direction\"=2,\"head_piece_bit\"=true]".to_string())];
            let r = case(bot, pad, hold, support, &set, STANDS[1].1, [0, -1, 0], Face::Up, [0.5, 1.0, 0.5]).await?;
            println!("{label:<34} S setblock-bed -> {r}");
        }
    }
    Ok(())
}

/// Clears the floor around T (support under T), sets `extra`, stands at `feet` facing T, places `hold` against
/// `against`'s `face` (offsets from T); returns what T, the cell above and its four neighbours show.
#[allow(clippy::too_many_arguments)]
async fn case(bot: &mut Bot, pad: &Pad, hold: &str, support: &str, extra: &[([i32; 3], String)], feet: [f32; 3], against: [i32; 3], face: Face, click: [f32; 3]) -> Result<String, Error> {
    let (lo, hi) = (pad.block(add(T, [-3, 0, -3])), pad.block(add(T, [3, 2, 3])));
    let (flo, fhi) = (pad.block(add(T, [-3, -1, -3])), pad.block(add(T, [3, -1, 3])));
    let s = pad.block(add(T, [0, -1, 0]));
    let mut cmds = vec![
        format!("/fill {} {} {} {} {} {} air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]),
        format!("/fill {} {} {} {} {} {} stone", flo[0], flo[1], flo[2], fhi[0], fhi[1], fhi[2]),
        format!("/setblock {} {} {} {support}", s[0], s[1], s[2]),
    ];
    for (off, block) in extra {
        let p = pad.block(add(T, *off));
        cmds.push(format!("/setblock {} {} {} {block}", p[0], p[1], p[2]));
    }
    for c in &cmds {
        pad.run(bot, c);
        bot.wait_ticks(2).await?;
    }
    let t = pad.block(T);
    let o = pad.block([0, 0, 0]);
    let (dx, dz) = (t[0] as f32 + 0.5 - (o[0] as f32 + feet[0]), t[2] as f32 + 0.5 - (o[2] as f32 + feet[2]));
    pad.teleport(bot, feet, (-dx).atan2(dz).to_degrees()).await?;
    let Some(slot) = bot.find_item(&format!("minecraft:{hold}")) else { return Ok("no item".into()) };
    if let Err(e) = bot.equip(slot, Destination::Hand).await {
        return Ok(format!("equip: {e}"));
    }
    let r = bot.place_block_at(pad.block(add(T, against)), face, click).await;
    // Ticks after the confirmed placement until the block entity shows (beds).
    let mut be_after = None;
    for k in 0..12 {
        if be_after.is_none() && bot.state().block_entities.get(pad.block(T)).is_some() {
            be_after = Some(k);
        }
        bot.wait_ticks(1).await?;
    }
    let show = |p: [i32; 3]| bot.block_state(p).map_or("unloaded".to_string(), |s| if s.is_air() { "-".into() } else { format!("{} [{}]", s.name.trim_start_matches("minecraft:"), s.properties) });
    let mut out = format!("T {} | up {}", show(t), show(add(t, [0, 1, 0])));
    for (n, off) in [("n", [0, 0, -1]), ("s", [0, 0, 1]), ("w", [-1, 0, 0]), ("e", [1, 0, 0])] {
        let p = add(t, off);
        if bot.block_state(p).is_some_and(|s| !s.is_air() && s.name != "minecraft:stone") {
            out += &format!(" | {n} {}", show(p));
        }
        if let Some(nbt) = bot.state().block_entities.get(p) {
            out += &format!(" | be-{n} {:?}", nbt.value.get("color"));
        }
    }
    if let Some(nbt) = bot.state().block_entities.get(t) {
        out += &format!(" | be {:?} after {be_after:?} ticks", nbt.value);
    }
    if let Err(e) = r {
        out += &format!(" | ERR {e}");
    }
    Ok(out)
}
