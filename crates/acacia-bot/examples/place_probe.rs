//! Probes how strict BDS sets a placed block's state: each block of the builder's set is placed into one cell
//! from each side, against each visible face, clicked low and high, and the resulting state is printed. Evidence
//! for `build/rules.rs`. `cargo run -p acacia-bot --example place_probe -- <server> <name>`; `ONLY=<item>`.
mod support;
#[path = "place_probe/twocell.rs"]
mod twocell;

use acacia_bot::interact::Face;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;
use support::{Error, Pad};

const ITEMS: &[&str] = &[
    "oak_stairs", "oak_slab", "oak_log", "furnace", "observer", "piston", "dispenser", "torch", "ladder", "stone_button", "cobblestone_wall", "oak_fence",
    "glass_pane",
];
/// `SET=oneclick`: blocks one click decides beyond the B1 set (docs/builder/oneclick.md), from every stand.
const ONECLICK: &[&str] = &[
    "trapdoor", "iron_trapdoor", "fence_gate", "lantern", "soul_lantern", "iron_chain", "stone_pressure_plate", "anvil", "hopper", "end_rod", "lightning_rod",
    "chest", "trapped_chest", "ender_chest", "barrel", "bell", "grindstone", "loom", "stonecutter_block", "smoker", "blast_furnace", "lectern", "campfire",
    "beehive", "white_glazed_terracotta", "amethyst_cluster", "decorated_pot", "chiseled_bookshelf", "redstone_torch",
];
/// `SET=oneclick` blocks whose state no click should change: probed from the east stand only.
const ONECLICK_PLAIN: &[&str] = &[
    "white_carpet", "oak_leaves", "hay_block", "redstone_block", "redstone_lamp", "sea_lantern", "dead_fire_coral_block", "cauldron", "composter",
    "copper_bulb", "tnt",
];
/// Chests and their kin placed side by side: does the second pair, and does it keep its own facing?
const PAIRS: &[&str] = &["chest", "trapped_chest", "ender_chest", "barrel"];
/// Side clicks into a cell over a fence or air, under stone or air (`SUPPORT=1`): what rests on what.
const SUPPORTED: &[&str] = &["lantern", "bell", "grindstone", "white_carpet", "stone_pressure_plate", "iron_chain", "end_rod"];
/// The target cell (pad offset); its neighbours are stone except the one between it and the bot.
const T: [i32; 3] = [5, 1, 0];

struct Stand {
    name: &'static str,
    /// Feet, pad offset.
    feet: [f32; 3],
    /// The neighbour of T left empty (between T and the bot), as an offset.
    near: [i32; 3],
}

const STANDS: &[Stand] = &[
    Stand { name: "N", feet: [5.5, 0.0, -1.5], near: [0, 0, -1] },
    Stand { name: "S", feet: [5.5, 0.0, 2.5], near: [0, 0, 1] },
    Stand { name: "W", feet: [3.5, 0.0, 0.5], near: [-1, 0, 0] },
    Stand { name: "E", feet: [7.5, 0.0, 0.5], near: [1, 0, 0] },
    Stand { name: "E-high", feet: [7.5, 2.0, 0.5], near: [1, 0, 0] },
    Stand { name: "E-low", feet: [7.5, -1.0, 0.5], near: [1, 0, 0] },
];
/// Within two blocks horizontally, above and below: where six-way blocks turn up or down (`VERT=1`).
const VERT_STANDS: &[Stand] = &[
    Stand { name: "E1-high", feet: [6.5, 2.0, 0.5], near: [0, 0, -1] },
    Stand { name: "E1-mid", feet: [6.5, 1.0, -0.5], near: [0, 0, -1] },
    Stand { name: "E1-low", feet: [6.5, -1.0, 0.5], near: [0, 0, -1] },
    Stand { name: "E2-high", feet: [7.4, 2.0, 0.5], near: [0, 0, -1] },
];
const VERT_ITEMS: &[&str] = &["piston", "dispenser", "observer"];
/// `SET=interact`: the interact tier's placements (use-clicks and sign rotation: `examples/interact_probe.rs`).
const INTERACT_ITEMS: &[&str] = &["repeater", "comparator", "lever", "daylight_detector", "oak_sign", "oak_hanging_sign", "white_candle", "sea_pickle", "turtle_egg"];

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Faces of T's neighbours the bot can see from `stand`, as (neighbour offset, face clicked, click offsets).
fn clicks(stand: &Stand) -> Vec<([i32; 3], Face, Vec<[f32; 3]>)> {
    let mut out = vec![([0, -1, 0], Face::Up, vec![[0.5, 1.0, 0.5]])];
    if stand.feet[1] + 1.62 < (T[1] - 1) as f32 + 1.0 + 1.0 {
        out.push(([0, 1, 0], Face::Down, vec![[0.5, 0.0, 0.5]]));
    }
    let sides = [([0, 0, -1], Face::South), ([0, 0, 1], Face::North), ([-1, 0, 0], Face::East), ([1, 0, 0], Face::West)];
    for (off, face) in sides.into_iter().filter(|(off, _)| *off != stand.near) {
        let o = face_offset(face);
        let base = [0.5 + 0.5 * o[0] as f32, 0.0, 0.5 + 0.5 * o[2] as f32];
        out.push((off, face, vec![[base[0], 0.25, base[2]], [base[0], 0.75, base[2]]]));
    }
    out
}

fn face_offset(f: Face) -> [i32; 3] {
    f.offset()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Probe".into());
    let only = std::env::var("ONLY").ok();
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.build(&mut bot, &[], 5).await?;
    if std::env::var("SET").as_deref() == Ok("twocell") {
        twocell::run(&mut bot, &pad, only.as_deref()).await?;
        pad.go_home(&mut bot).await?;
        bot.disconnect().await;
        return Ok(());
    }
    let vert = std::env::var("VERT").is_ok();
    let oneclick = std::env::var("SET").is_ok_and(|s| s == "oneclick");
    if oneclick && std::env::var("SUPPORT").is_ok() {
        for item in SUPPORTED.iter().filter(|i| only.as_deref().is_none_or(|o| o == **i)) {
            for c in ["/clear @s".to_string(), format!("/give @s {item} 64")] {
                pad.run(&bot, &c);
                bot.wait_ticks(4).await?;
            }
            for (below, above) in [("oak_fence", "stone"), ("air", "stone"), ("oak_fence", "air"), ("air", "air")] {
                for y in [0.25, 0.75] {
                    let extra = [([0, -1, 0], below), ([0, 1, 0], above)];
                    let r = probe_with(&mut bot, &pad, item, &STANDS[3], add(T, [-1, 0, 0]), Face::East, [1.0, y, 0.5], &extra).await?;
                    println!("{item:<16} below {below} above {above} East@{y:.2} -> {r}");
                }
            }
            if *item == "bell" {
                // Does a side bell turn `multiple` when the cell across from its wall fills later?
                let r = probe(&mut bot, &pad, item, &STANDS[3], add(T, [-1, 0, 0]), Face::East, [1.0, 0.5, 0.5]).await?;
                pad.teleport(&mut bot, [9.5, 0.0, 0.5], 90.0).await?;
                let e = pad.block(add(T, [1, 0, 0]));
                pad.run(&bot, &format!("/setblock {} {} {} stone", e[0], e[1], e[2]));
                bot.wait_ticks(10).await?;
                let after = bot.block_state(pad.block(T)).map_or("unloaded".to_string(), |s| s.properties.to_string());
                println!("bell side {r}; east filled -> [{after}]");
            }
            // Chain ends as the clicked block: a chandelier's lantern under a chain, a chain on a chain.
            for (off, face, click) in [([0, 1, 0], Face::Down, [0.5, 0.0, 0.5]), ([0, -1, 0], Face::Up, [0.5, 1.0, 0.5])] {
                let extra = [(off, "iron_chain")];
                let r = probe_with(&mut bot, &pad, item, &STANDS[0], add(T, off), face, click, &extra).await?;
                println!("{item:<16} on chain {face:?} -> {r}");
            }
        }
        pad.go_home(&mut bot).await?;
        bot.disconnect().await;
        return Ok(());
    }
    if oneclick && vert {
        // T's sides open so the eye sees the face over or under it (the shared sight rule hides it otherwise).
        let sides = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
        for item in ["barrel"] {
            for c in ["/clear @s".to_string(), format!("/give @s {item} 64")] {
                pad.run(&bot, &c);
                bot.wait_ticks(4).await?;
            }
            for stand in VERT_STANDS {
                let (face, against, open) = if stand.feet[1] > 0.0 { (Face::Up, [0, -1, 0], [0, 1, 0]) } else { (Face::Down, [0, 1, 0], [0, -1, 0]) };
                let extra: Vec<([i32; 3], &str)> = sides.iter().chain([&open]).map(|o| (*o, "air")).collect();
                let click = if face == Face::Up { [0.5, 1.0, 0.5] } else { [0.5, 0.0, 0.5] };
                let r = probe_with(&mut bot, &pad, item, stand, add(T, against), face, click, &extra).await?;
                println!("{item:<16} {:<7} {face:?} open sides -> {r}", stand.name);
            }
        }
        pad.go_home(&mut bot).await?;
        bot.disconnect().await;
        return Ok(());
    }
    if oneclick && std::env::var("PAIR").is_ok() {
        for item in PAIRS.iter().filter(|i| only.as_deref().is_none_or(|o| o == **i)) {
            pair(&mut bot, &pad, item).await?;
        }
        pad.go_home(&mut bot).await?;
        bot.disconnect().await;
        return Ok(());
    }
    let plain: Vec<&str> = if oneclick { ONECLICK_PLAIN.to_vec() } else { Vec::new() };
    let (items, stands) = match () {
        _ if vert => (VERT_ITEMS, VERT_STANDS),
        _ if oneclick => ([ONECLICK, ONECLICK_PLAIN].concat().leak() as &[&str], STANDS),
        _ if std::env::var("SET").as_deref() == Ok("interact") => (INTERACT_ITEMS, STANDS),
        _ => (ITEMS, STANDS),
    };
    for item in items.iter().filter(|i| only.as_deref().is_none_or(|o| o == **i)) {
        let stands = if plain.contains(item) { &stands[3..4] } else { stands };
        for c in ["/clear @s".to_string(), format!("/give @s {item} 64")] {
            pad.run(&bot, &c);
            bot.wait_ticks(4).await?;
        }
        for stand in stands {
            let faces = if stand.name.contains('-') && !vert { clicks(stand).into_iter().take(1).collect() } else { clicks(stand) };
            for (off, face, points) in faces {
                for click in points {
                    let result = probe(&mut bot, &pad, item, stand, add(T, off), face, click).await?;
                    println!("{item:<16} {:<6} {face:?}@{:.2} -> {result}", stand.name, click[1]);
                }
            }
        }
        if *item == "oak_slab" && !vert {
            let stand = &STANDS[3];
            let first = probe(&mut bot, &pad, item, stand, add(T, [0, -1, 0]), Face::Up, [0.5, 1.0, 0.5]).await?;
            let again = place(&mut bot, &pad, add(T, [0, -1, 0]), Face::Up, [0.5, 1.0, 0.5], item).await;
            let side = place(&mut bot, &pad, add(T, [-1, 0, 0]), Face::East, [1.0, 0.75, 0.5], item).await;
            println!("oak_slab double: first {first}; again-up {again}; then-side {side}");
        }
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

/// Resets the cell and its neighbours, stands at `stand`, places `item` against `against`'s `face`.
async fn probe(bot: &mut Bot, pad: &Pad, item: &str, stand: &Stand, against: [i32; 3], face: Face, click: [f32; 3]) -> Result<String, Error> {
    probe_with(bot, pad, item, stand, against, face, click, &[]).await
}

/// [`probe`] with `extra` cells (offsets from T) set after the reset.
#[allow(clippy::too_many_arguments)]
async fn probe_with(bot: &mut Bot, pad: &Pad, item: &str, stand: &Stand, against: [i32; 3], face: Face, click: [f32; 3], extra: &[([i32; 3], &str)]) -> Result<String, Error> {
    let (lo, hi) = (pad.block(add(T, [-1, -1, -1])), pad.block(add(T, [1, 1, 1])));
    let near = pad.block(add(T, stand.near));
    let [t0, t1, t2] = pad.block(T);
    let mut cmds = vec![
        format!("/fill {} {} {} {} {} {} stone", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]),
        format!("/setblock {t0} {t1} {t2} air"),
        format!("/setblock {} {} {} air", near[0], near[1], near[2]),
    ];
    // Clear headroom around the stand (high and low stands need their own floor).
    let f = stand.feet.map(|v| v.floor() as i32);
    let s = pad.block(f);
    cmds.push(format!("/fill {} {} {} {} {} {} air", s[0], s[1], s[2], s[0], s[1] + 1, s[2]));
    cmds.push(format!("/setblock {} {} {} stone", s[0], s[1] - 1, s[2]));
    for (off, block) in extra {
        let p = pad.block(add(T, *off));
        cmds.push(format!("/setblock {} {} {} {block}", p[0], p[1], p[2]));
    }
    for c in &cmds {
        pad.run(bot, c);
        bot.wait_ticks(2).await?;
    }
    let yaw = {
        let (dx, dz) = (t0 as f32 + 0.5 - (pad.block([0, 0, 0])[0] as f32 + stand.feet[0]), t2 as f32 + 0.5 - (pad.block([0, 0, 0])[2] as f32 + stand.feet[2]));
        (-dx).atan2(dz).to_degrees()
    };
    pad.teleport(bot, stand.feet, yaw).await?;
    Ok(place(bot, pad, against, face, click, item).await)
}

/// Two of `item` side by side on the floor from the west stand (both facing west: a pairing row along z), then
/// a third south of them from a stand south of it (facing south). Prints each cell's state and pairing NBT.
async fn pair(bot: &mut Bot, pad: &Pad, item: &str) -> Result<(), Error> {
    let (lo, hi, a, b) = (pad.block([3, 0, -2]), pad.block([7, 3, 4]), pad.block([5, 0, 0]), pad.block([5, 0, 2]));
    for c in [
        "/clear @s".to_string(),
        format!("/give @s {item} 64"),
        format!("/fill {} {} {} {} {} {} air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]),
        format!("/fill {} {} {} {} {} {} stone", a[0], a[1], a[2], b[0], b[1], b[2]),
    ] {
        pad.run(bot, &c);
        bot.wait_ticks(4).await?;
    }
    let cells = [add(T, [0, 0, 0]), add(T, [0, 0, 1]), add(T, [0, 0, 2])];
    let stands: [([f32; 3], f32); 3] = [([3.5, 0.0, 0.5], -90.0), ([3.5, 0.0, 1.5], -90.0), ([5.5, 0.0, 4.5], 180.0)];
    for (cell, (feet, yaw)) in cells.iter().zip(stands) {
        pad.teleport(bot, feet, yaw).await?;
        let r = place(bot, pad, add(*cell, [0, -1, 0]), Face::Up, [0.5, 1.0, 0.5], item).await;
        println!("{item:<16} pair place {cell:?} -> {r}");
    }
    bot.wait_ticks(10).await?;
    for cell in cells {
        let p = pad.block(cell);
        let state = bot.block_state(p).map_or("unloaded".to_string(), |s| format!("{} [{}]", s.name, s.properties));
        let nbt = bot.state().block_entities.get(p);
        let keys = ["pairx", "pairz", "pairlead", "Findable"].map(|k| format!("{k}={:?}", nbt.and_then(|n| n.value.get(k))));
        println!("{item:<16} pair {cell:?}: {state} {}", keys.join(" "));
    }
    Ok(())
}

async fn place(bot: &mut Bot, pad: &Pad, against: [i32; 3], face: Face, click: [f32; 3], item: &str) -> String {
    let Some(slot) = bot.find_item(&format!("minecraft:{item}")) else { return "no item".into() };
    if let Err(e) = bot.equip(slot, Destination::Hand).await {
        return format!("equip: {e}");
    }
    let r = bot.place_block_at(pad.block(against), face, click).await;
    let _ = bot.wait_ticks(4).await;
    let (yaw, pitch) = bot.facing();
    let cell = bot.block_state(pad.block(T)).map_or("unloaded".to_string(), |s| format!("{} [{}]", s.name, s.properties));
    match r {
        Ok(()) => format!("{cell} (yaw {yaw:.0} pitch {pitch:.0})"),
        Err(e) => format!("ERR {e}; cell {cell}"),
    }
}
