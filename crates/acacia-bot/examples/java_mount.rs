//! Java bot riding: mounts the nearest `ACACIA_MOUNT` (default pig) and drives a fixed course (ahead, a turn, a
//! charged jump, ahead, stop), printing the mount every 10 ticks and the server's corrections, then gets off.
//! Saddle it first, and for a pig or strider put the stick in the bot's hand.
//! `cargo run -p acacia-bot --features java --example java_mount -- <host> <port> [name]`
//! `ACACIA_STAY` keeps the bot seated at the end, to tell the mount's alerts from the dismount's.

use acacia_bot::{Bot, BotConfig};

const FIND_TICKS: u32 = 400;
const REACH: f64 = 3.5;
const SETTLE_TICKS: u32 = 40;
/// (ticks, forward, yaw turn per tick, jump held).
const COURSE: [(u32, f32, f32, bool); 6] =
    [(40, 1.0, 0.0, false), (30, 1.0, 3.0, false), (12, 1.0, 0.0, true), (30, 1.0, 0.0, false), (20, 0.0, 0.0, false), (20, -1.0, 0.0, false)];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_mount <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Rider", String::as_str);
    let kind = std::env::var("ACACIA_MOUNT").unwrap_or_else(|_| "pig".into());
    let mut config = BotConfig::default();
    config.trackers.entities = true;
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, config).await?;
    let mut mount = None;
    for _ in 0..FIND_TICKS {
        bot.wait_ticks(1).await?;
        mount = nearest(&bot, &kind);
        if mount.is_some() {
            break;
        }
    }
    let mount = mount.ok_or(format!("no {kind} in reach"))?;
    bot.wait_ticks(SETTLE_TICKS).await?;
    println!("mount {:?}", bot.mount(mount).await);
    bot.wait_ticks(5).await?;
    let mut t = 0;
    for (ticks, forward, turn, jump) in COURSE {
        for _ in 0..ticks {
            if let Some(c) = bot.controls() {
                (c.forward, c.jump) = (forward, jump);
                c.yaw += turn;
            }
            bot.wait_ticks(1).await?;
            t += 1;
            if t % 10 == 0 || jump {
                print_mount(&bot, t);
            }
        }
    }
    if std::env::var("ACACIA_STAY").is_err() {
        println!("dismount {:?}", bot.dismount().await);
    }
    bot.wait_ticks(40).await?;
    println!("off: riding {} corrections {:?}", bot.vehicle().is_some(), bot.boat_corrections());
    bot.disconnect().await;
    Ok(())
}

fn nearest(bot: &Bot, kind: &str) -> Option<u64> {
    let eye = bot.eye_position();
    let apart = |e: &acacia_bot::state::Entity| {
        let f = e.feet();
        ((f.x - eye[0]).powi(2) + (f.y - eye[1]).powi(2) + (f.z - eye[2]).powi(2)).sqrt()
    };
    bot.state().entities.iter().filter(|e| e.kind.ends_with(kind) && f64::from(apart(e)) < REACH).map(|e| e.runtime_id).next()
}

fn print_mount(bot: &Bot, t: u32) {
    match bot.java_mount() {
        Some(m) => println!(
            "t {t} {:?} {:.4?} yaw {:.2} ground {} delta {:.4?} cooldown {} corrections {:?}",
            m.kind, m.body.pos, m.body.yaw, m.body.on_ground, m.body.delta, m.dash_cooldown, bot.boat_corrections()
        ),
        None => println!("t {t} not driving; vehicle {:?}", bot.vehicle()),
    }
}
