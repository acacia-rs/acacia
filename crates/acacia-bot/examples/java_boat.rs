//! Java bot boating: boards the nearest boat, paddles a fixed course (ahead, a left turn, ahead, turning on the spot,
//! backwards, coasting), printing the boat every 10 ticks and the server's corrections, then gets off.
//! Put it next to a boat with ~35 blocks of open water ahead (south); a boat on the bank paddles over land and drops in.
//! `cargo run -p acacia-bot --features java --example java_boat -- <host> <port> [name]`

use acacia_bot::{Bot, BotConfig};

/// Ticks to wait for a boat within reach.
const FIND_TICKS: u32 = 400;
const REACH: f32 = 3.0;
/// Ticks for a summoned boat to come to rest before boarding.
const SETTLE_TICKS: u32 = 60;
/// (ticks, strafe, forward): strafe > 0 is left.
const COURSE: [(u32, f32, f32); 6] = [(80, 0.0, 1.0), (30, 1.0, 1.0), (20, 0.0, 1.0), (20, -1.0, 0.0), (15, 0.0, -1.0), (30, 0.0, 0.0)];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_boat <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Boater", String::as_str);
    let mut config = BotConfig::default();
    config.trackers.entities = true;
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, config).await?;
    let mut boat = None;
    for _ in 0..FIND_TICKS {
        bot.wait_ticks(1).await?;
        boat = nearest_boat(&bot);
        if boat.is_some() {
            break;
        }
    }
    let boat = boat.ok_or("no boat in reach")?;
    bot.wait_ticks(SETTLE_TICKS).await?;
    println!("mount {:?}", bot.mount(boat).await);
    println!("boarded: boat {:?} player {:.4?}", bot.java_boat().map(|b| (b.pos, b.yaw, b.status)), player(&bot));
    let (mut t, mut status) = (0, None);
    for (ticks, strafe, forward) in COURSE {
        if let Some(c) = bot.controls() {
            (c.strafe, c.forward) = (strafe, forward);
        }
        for _ in 0..ticks {
            bot.wait_ticks(1).await?;
            t += 1;
            let now = bot.java_boat().map(|b| b.status);
            if t % 10 == 0 || now != status {
                print_boat(&bot, t);
            }
            status = now;
        }
    }
    println!("dismount {:?}", bot.dismount().await);
    bot.wait_ticks(40).await?;
    println!("off: riding {} player {:.4?} corrections {:?}", bot.vehicle().is_some(), player(&bot), bot.boat_corrections());
    bot.disconnect().await;
    Ok(())
}

fn nearest_boat(bot: &Bot) -> Option<u64> {
    let eye = bot.eye_position();
    let apart = |e: &acacia_bot::state::Entity| {
        let f = e.feet();
        ((f.x - eye[0]).powi(2) + (f.y - eye[1]).powi(2) + (f.z - eye[2]).powi(2)).sqrt()
    };
    bot.state().entities.iter().filter(|e| e.kind.ends_with("boat") && apart(e) < REACH).map(|e| e.runtime_id).next()
}

fn print_boat(bot: &Bot, t: u32) {
    match bot.java_boat() {
        Some(b) => println!("t {t} boat {:.4?} yaw {:.2} {:?} corrections {:?}", b.pos, b.yaw, b.status, bot.boat_corrections()),
        None => println!("t {t} not driving; vehicle {:?}", bot.vehicle()),
    }
}

fn player(bot: &Bot) -> [f64; 3] {
    bot.java().map_or([f64::NAN; 3], |j| j.client().movement.player.pos)
}
