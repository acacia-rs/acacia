//! Which effect a Bedrock suspicious stew gives by its aux value: each value is given, held, eaten, and the
//! `MobEffect`s the server then shows are printed.
//! `cargo run -p acacia-bot --example stew_probe -- <server> [name] [max aux]`
mod support;

use acacia_bot::items::SlotRef;
use acacia_bot::survival::Destination;
use support::Error;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Probe".into());
    let max: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(14);
    let (mut bot, _pad) = support::connect(&server, &name).await?;
    // AUX=5,6,13 picks the values; peaceful stops hunger, so the drain runs on easy
    let list: Vec<u32> = std::env::var("AUX").map_or_else(|_| (0..=max).collect(), |v| v.split(',').filter_map(|a| a.parse().ok()).collect());
    bot.command("/difficulty easy");
    for aux in list {
        for cmd in ["/effect @s clear".to_string(), "/clear @s".into(), format!("/give @s suspicious_stew 1 {aux}")] {
            bot.command(&cmd);
            bot.wait_ticks(6).await?;
        }
        // Hunger drains saturation then food, so a saturation effect shows against the stew's own 6 + 7.2
        bot.command("/effect @s hunger 2 60 true");
        bot.wait_ticks(60).await?;
        bot.command("/effect @s clear");
        bot.wait_ticks(10).await?;
        let (food, sat) = (bot.state().player.hunger, bot.state().player.saturation);
        let Some(slot) = bot.find_item_named("minecraft:suspicious_stew") else {
            println!("aux {aux}: no stew in the inventory");
            continue;
        };
        if let SlotRef::Main(i) = slot {
            let s = &bot.state().inventory.main[usize::from(i)];
            println!("aux {aux}: stack metadata {} nbt {:?}", s.metadata, s.nbt);
        }
        bot.equip(slot, Destination::Hand).await?;
        let eaten = bot.consume().await;
        bot.wait_ticks(10).await?;
        let effects: Vec<_> = bot.state().player.effects.iter().map(|e| (e.id, e.amplifier, e.duration)).collect();
        let p = &bot.state().player;
        println!("aux {aux}: eat {eaten:?}, effects (id, amplifier, ticks) {effects:?}, food {food} -> {}, saturation {sat} -> {}", p.hunger, p.saturation);
    }
    Ok(())
}
