//! Probes the interact tier's states on strict BDS (evidence for `build/rules/interact.rs`): what use-clicks do
//! (`PART=use`), extra placements into a held cell (`count`), standing-sign rotation by look and sign text
//! (`sign`), hanging signs (`hanging`). Placement directions: `place_probe` with `SET=interact`.
//! `cargo run -p acacia-bot --example interact_probe -- <server> <name> <observer>`
mod support;

use acacia_bot::interact::Face;
use acacia_bot::signs::SignSide;
use acacia_bot::survival::Destination;
use acacia_bot::Bot;
use support::{Error, Pad};

const T: [i32; 3] = [5, 0, 0];

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cell(bot: &Bot, p: [i32; 3]) -> String {
    bot.block_state(p).map_or("unloaded".into(), |s| format!("{} [{}]", s.name, s.properties))
}

async fn run(bot: &mut Bot, pad: &Pad, cmds: &[String]) -> Result<(), Error> {
    for c in cmds {
        pad.run(bot, c);
        bot.wait_ticks(3).await?;
    }
    Ok(())
}

async fn hold(bot: &mut Bot, pad: &Pad, item: &str) -> Result<(), Error> {
    run(bot, pad, &["/clear @s".into(), format!("/give @s {item} 64")]).await?;
    if let Some(slot) = bot.find_item(&format!("minecraft:{item}")) {
        bot.equip(slot, Destination::Hand).await?;
    }
    Ok(())
}

fn setblock(pad: &Pad, p: [i32; 3], block: &str) -> String {
    let [x, y, z] = pad.block(p);
    format!("/setblock {x} {y} {z} {block}")
}

/// Air around T (two high), stone under it; the bot north of T, facing it.
async fn reset(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    let (lo, hi) = (pad.block(add(T, [-2, 0, -2])), pad.block(add(T, [2, 2, 2])));
    run(bot, pad, &[format!("/fill {} {} {} {} {} {} air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]), setblock(pad, add(T, [0, -1, 0]), "stone")]).await?;
    pad.teleport(bot, [5.5, 0.0, -1.5], 0.0).await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Probe".into());
    let observer = args.next().unwrap_or_else(|| "ProbeObs".into());
    let part = std::env::var("PART").unwrap_or_else(|_| "use,count,sign,hanging".into());
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.build(&mut bot, &[], 5).await?;
    if part.contains("use") {
        uses(&mut bot, &pad).await?;
    }
    if part.contains("count") {
        counts(&mut bot, &pad).await?;
    }
    let mut signs = Vec::new();
    if part.contains("sign") {
        signs = rotations(&mut bot, &pad).await?;
    }
    if part.contains("hanging") {
        hanging(&mut bot, &pad).await?;
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    if !signs.is_empty() {
        let (mut o, _) = support::connect(&server, &observer).await?;
        pad.teleport(&mut o, [10.0, 3.0, 0.0], 0.0).await?;
        o.wait_ticks(60).await?;
        for p in signs {
            let s = &o.state().block_entities;
            println!("observer sign {p:?}: {} front {:?} back {:?}", cell(&o, p), s.sign_text(p, true), s.sign_text(p, false));
        }
        o.disconnect().await;
    }
    Ok(())
}

/// Each block set by command, then used five times from the north, empty-handed and holding dirt.
async fn uses(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    let blocks = [
        "unpowered_repeater", "unpowered_comparator", "lever [\"lever_direction\"=\"up_north_south\"]", "daylight_detector", "oak_trapdoor", "oak_fence_gate", "oak_door",
        "iron_trapdoor", "white_candle", "noteblock", "copper_door",
    ];
    for held in ["air", "dirt"] {
        for b in blocks {
            reset(bot, pad).await?;
            run(bot, pad, &[setblock(pad, T, b)]).await?;
            hold(bot, pad, held).await?;
            let mut seen = vec![cell(bot, pad.block(T))];
            for _ in 0..5 {
                let r = bot.use_item_on_block(pad.block(T), Face::North).await;
                bot.wait_ticks(5).await?;
                seen.push(match r {
                    Ok(()) => cell(bot, pad.block(T)),
                    Err(e) => format!("ERR {e}"),
                });
            }
            println!("use {b} holding {held}: {}", seen.join(" -> "));
            if b.contains("door") && !b.contains("trap") {
                println!("  upper {}", cell(bot, pad.block(add(T, [0, 1, 0]))));
            }
        }
    }
    Ok(())
}

/// A first item onto the floor, then more: into the held cell against the floor, and against the block itself.
async fn counts(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    for item in ["white_candle", "sea_pickle", "turtle_egg"] {
        for floor in ["stone", "sand"] {
            for how in ["floor", "itself-up", "itself-side"] {
                reset(bot, pad).await?;
                run(bot, pad, &[setblock(pad, add(T, [0, -1, 0]), floor)]).await?;
                hold(bot, pad, item).await?;
                let below = pad.block(add(T, [0, -1, 0]));
                let mut seen = Vec::new();
                for _ in 0..5 {
                    let first = bot.block_state(pad.block(T)).is_none_or(|s| s.is_air());
                    let r = match how {
                        _ if first => bot.place_block_at(below, Face::Up, [0.5, 1.0, 0.5]).await,
                        "floor" => bot.place_block_at(below, Face::Up, [0.3, 1.0, 0.3]).await,
                        "itself-up" => bot.place_block_at(pad.block(T), Face::Up, [0.5, 0.4, 0.5]).await,
                        _ => bot.place_block_at(pad.block(T), Face::North, [0.5, 0.2, 0.4]).await,
                    };
                    bot.wait_ticks(5).await?;
                    seen.push(match r {
                        Ok(()) => cell(bot, pad.block(T)),
                        Err(e) => format!("ERR {e}; {}", cell(bot, pad.block(T))),
                    });
                }
                println!("count {item} on {floor} by {how}: {}; above {}", seen.join(" -> "), cell(bot, pad.block(add(T, [0, 1, 0]))));
            }
        }
    }
    Ok(())
}

/// A sign on the floor from 32 looks (11.25 degrees apart), each in its own cell, with front text; the first
/// also gets back text from the far side. Then wall signs on each side face. Returns the sign cells.
async fn rotations(bot: &mut Bot, pad: &Pad) -> Result<Vec<[i32; 3]>, Error> {
    hold(bot, pad, "oak_sign").await?;
    let mut cells = Vec::new();
    for k in 0..32 {
        let c = [(k % 4) * 5, 0, -10 + (k / 4) * 3];
        let theta = (k as f32 * 11.25).to_radians();
        let feet = [c[0] as f32 + 0.5 + 2.2 * theta.cos(), 0.0, c[2] as f32 + 0.5 + 2.2 * theta.sin()];
        let yaw = (-(c[0] as f32 + 0.5 - feet[0])).atan2(c[2] as f32 + 0.5 - feet[2]).to_degrees();
        pad.teleport(bot, feet, yaw).await?;
        let below = pad.block(add(c, [0, -1, 0]));
        let r = bot.place_block_at(below, Face::Up, [0.5, 1.0, 0.5]).await;
        bot.wait_ticks(4).await?;
        let p = pad.block(c);
        let editor = bot.state().signs.editor;
        let wrote = match editor {
            Some(e) if e.position == p => bot.write_open_sign(&format!("k={k}\nline two")).map_err(|e| e.to_string()),
            _ => Err("no editor".into()),
        };
        bot.wait_ticks(4).await?;
        let (look, _) = bot.facing();
        println!("sign k {k} theta {:.2} yaw {look:.2}: {r:?} {} editor {editor:?} wrote {wrote:?}", k as f32 * 11.25, cell(bot, p));
        if k == 0 {
            let back = [c[0] as f32 + 0.5 - 2.2, 0.0, c[2] as f32 + 0.5];
            pad.teleport(bot, back, -90.0).await?;
            let r = bot.write_sign(p, SignSide::Back, "the back").await;
            bot.wait_ticks(4).await?;
            println!("sign k 0 back: {r:?}");
        }
        cells.push(p);
    }
    for (off, face) in [([0, 0, -1], Face::South), ([0, 0, 1], Face::North), ([-1, 0, 0], Face::East), ([1, 0, 0], Face::West)] {
        let c = [20, 0, (off[2] + off[0] * 2) * 3];
        let support = add(c, off);
        run(bot, pad, &[setblock(pad, support, "stone")]).await?;
        let o = face.offset();
        let feet = [c[0] as f32 + 0.5 + o[0] as f32 * 1.5, 0.0, c[2] as f32 + 0.5 + o[2] as f32 * 1.5];
        pad.teleport(bot, feet, 0.0).await?;
        let r = bot.place_block_at(pad.block(support), face, [0.5 + 0.5 * o[0] as f32, 0.5, 0.5 + 0.5 * o[2] as f32]).await;
        bot.wait_ticks(4).await?;
        if bot.state().signs.editor.is_some() {
            let _ = bot.write_open_sign("wall");
        }
        println!("wall sign on {face:?}: {r:?} {}", cell(bot, pad.block(c)));
        cells.push(pad.block(c));
    }
    Ok(cells)
}

/// Hanging signs under a block from four looks and on each side face of a block.
async fn hanging(bot: &mut Bot, pad: &Pad) -> Result<(), Error> {
    for (name, feet) in [("N", [5.5, 0.0, -1.5]), ("S", [5.5, 0.0, 2.5]), ("W", [3.5, 0.0, 0.5]), ("E", [7.5, 0.0, 0.5]), ("NE-diag", [7.0, 0.0, -1.0])] {
        reset(bot, pad).await?;
        run(bot, pad, &[setblock(pad, add(T, [0, 2, 0]), "stone")]).await?;
        hold(bot, pad, "oak_hanging_sign").await?;
        pad.teleport(bot, feet, 0.0).await?;
        let r = bot.place_block_at(pad.block(add(T, [0, 2, 0])), Face::Down, [0.5, 0.0, 0.5]).await;
        bot.wait_ticks(4).await?;
        let (yaw, _) = bot.facing();
        println!("hanging under from {name} yaw {yaw:.0}: {r:?} {}", cell(bot, pad.block(add(T, [0, 1, 0]))));
    }
    for (off, face, feet) in [([0, 0, -1], Face::South, [5.5, 0.0, 1.8]), ([-1, 0, 0], Face::East, [6.8, 0.0, 0.5])] {
        reset(bot, pad).await?;
        run(bot, pad, &[setblock(pad, add(T, off), "stone")]).await?;
        hold(bot, pad, "oak_hanging_sign").await?;
        pad.teleport(bot, feet, 0.0).await?;
        let o = face.offset();
        let r = bot.place_block_at(pad.block(add(T, off)), face, [0.5 + 0.5 * o[0] as f32, 0.5, 0.5 + 0.5 * o[2] as f32]).await;
        bot.wait_ticks(4).await?;
        println!("hanging on side {face:?}: {r:?} {}", cell(bot, pad.block(T)));
    }
    Ok(())
}
