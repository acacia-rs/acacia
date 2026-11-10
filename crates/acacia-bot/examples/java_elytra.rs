//! Java bot gliding: once teleported above y -20 with an elytra and firework rockets in the hotbar, it puts the
//! elytra on, opens it, glides north-east looking slightly down, boosts with a rocket twice and glides until it lands,
//! printing where it is every 10 ticks.
//! `cargo run -p acacia-bot --features java --example java_elytra -- <host> <port> [name]`

use acacia_bot::{Bot, BotConfig};
use acacia_world::neutral::Item;

/// Ticks to wait for the teleport before giving up.
const TELEPORT_TICKS: u32 = 400;
/// The gate's flat ground is at -60; the teleport puts the bot far above it.
const AIRBORNE_ABOVE: f64 = -20.0;
const BOOSTS_AT: [u32; 2] = [5, 50];
const MAX_TICKS: u32 = 600;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_elytra <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Glider", String::as_str);
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    for _ in 0..TELEPORT_TICKS {
        bot.wait_ticks(1).await?;
        let given = bot.state().inventory.main.iter().any(|s| bot.state().item(s) == Some(Item::Elytra));
        if given && position(&bot)[1] > AIRBORNE_ABOVE {
            break;
        }
    }
    println!("equip {:?}", bot.equip_elytra().await);
    let from = position(&bot);
    println!("start {from:.2?} elytra worn {}", bot.wears_elytra());
    if let Some(c) = bot.controls() {
        (c.yaw, c.pitch) = (-135.0, 10.0);
    }
    println!("glide {:?}", bot.start_gliding().await);
    for t in 0..MAX_TICKS {
        if BOOSTS_AT.contains(&t) {
            println!("boost at {t} {:?}", bot.boost_with_firework().await);
        }
        bot.wait_ticks(1).await?;
        if t % 10 == 0 {
            println!("t {t} {:.2?} gliding {}", position(&bot), bot.is_gliding());
        }
        if !bot.is_gliding() {
            break;
        }
    }
    bot.wait_ticks(20).await?;
    let to = position(&bot);
    println!("landed {to:.2?} flew {:.1}", (to[0] - from[0]).hypot(to[2] - from[2]));
    bot.disconnect().await;
    Ok(())
}

fn position(bot: &Bot) -> [f64; 3] {
    bot.java().map_or([f64::NAN; 3], |j| j.client().movement.player.pos)
}
