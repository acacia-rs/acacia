//! A second player for viewer screenshots, on a flat-world server where the bot is operator: it
//! stands at a spot facing north, leashes a cow it summons beside itself, then sneaks with a
//! shield in its off hand (Bedrock's way to block) for a minute.
//! `cargo run -p acacia-bot --example props -- <server> [x y z of its feet]`
use acacia_bot::client::Client;
use acacia_bot::{Bot, BotConfig};

type Error = Box<dyn std::error::Error>;

const COW: &str = "minecraft:cow";
/// Ticks the pose is held for a screenshot.
const HOLD: u32 = 1200;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19174".into());
    let mut coordinate = |default: i32| args.next().and_then(|a| a.parse().ok()).unwrap_or(default);
    let [x, y, z] = [coordinate(240), coordinate(-60), coordinate(104)];
    let config = BotConfig { physics: true, ..BotConfig::default() };
    let mut bot = Bot::connect(Client::builder(&server).offline("Prop"), config).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    // A bot that died here last time respawns at the world spawn first.
    bot.wait_ticks(100).await?;
    let scene = [
        "/gamemode creative @s".to_owned(),
        format!("/tp @s {} {y} {} 180 0", x as f32 + 0.5, z as f32 + 0.5),
        format!("/kill @e[type=cow,x={x},y={y},z={z},r=12]"),
        "/clear @s".to_owned(),
        "/replaceitem entity @s slot.weapon.offhand 0 shield".to_owned(),
        "/replaceitem entity @s slot.hotbar 0 lead".to_owned(),
        format!("/summon cow {} {y} {}", x as f32 + 2.5, z as f32 + 0.5),
    ];
    for command in &scene {
        bot.client().command(command);
        bot.wait_ticks(6).await?;
    }
    bot.select_hotbar(0)?;
    bot.wait_ticks(20).await?;
    let cow = bot.state().entities.iter().find(|e| e.kind == COW).map(|e| e.runtime_id).ok_or("no cow was tracked")?;
    bot.interact_entity(cow).await?;
    bot.wait_ticks(10).await?;
    println!("leashed cow {cow}; holding {:?}", bot.state().inventory_summary());
    // Turned back to face north after aiming at the cow.
    bot.client().command(&format!("/tp @s {} {y} {} 180 0", x as f32 + 0.5, z as f32 + 0.5));
    if let Some(controls) = bot.controls() {
        controls.sneak = true;
    }
    println!("posed");
    Ok(bot.wait_ticks(HOLD).await?)
}
