//! Probes how strict BDS sets a placed block's state: each block of the builder's set is placed into one cell
//! from each side, against each visible face, clicked low and high, and the resulting state is printed. Evidence
//! for `build/rules.rs`. `cargo run -p acacia-bot --example place_probe -- <server> <name>`; `ONLY=<item>`.
mod support;

use acacia_bot::interact::Face;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;
use support::{Error, Pad};

const ITEMS: &[&str] = &[
    "oak_stairs", "oak_slab", "oak_log", "furnace", "observer", "piston", "dispenser", "torch", "ladder", "stone_button", "cobblestone_wall", "oak_fence",
    "glass_pane",
];
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
    let vert = std::env::var("VERT").is_ok();
    let (items, stands) = match () {
        _ if vert => (VERT_ITEMS, VERT_STANDS),
        _ if std::env::var("SET").as_deref() == Ok("interact") => (INTERACT_ITEMS, STANDS),
        _ => (ITEMS, STANDS),
    };
    for item in items.iter().filter(|i| only.as_deref().is_none_or(|o| o == **i)) {
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
