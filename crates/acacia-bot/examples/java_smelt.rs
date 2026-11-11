//! Java bot: smelts 3 raw iron with 1 coal at the furnace at `<x> <y> <z>` and prints the inventory. Give it the
//! raw iron and coal first.
//! `cargo run -p acacia-bot --features java --example java_smelt -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::{Bot, BotConfig};

fn inventory(bot: &Bot) -> Vec<String> {
    let state = bot.state();
    let named = |s: &acacia_bot::state::ItemStack| format!("{}x{}", state.items.name(s.network_id).unwrap_or("?"), s.count);
    state.inventory.main.iter().filter(|s| !s.is_empty()).map(named).collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_smelt <host> <port> <x> <y> <z> [name]".into()) };
    let furnace = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Smelter", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    println!("start {:?}", inventory(&bot));
    let started = std::time::Instant::now();
    let taken = bot.smelt(furnace, "minecraft:raw_iron", 3, Some(("minecraft:coal", 1))).await;
    bot.wait_ticks(10).await?;
    println!("smelt {taken:?} after {:.1} s -> {:?}", started.elapsed().as_secs_f32(), inventory(&bot));
    bot.disconnect().await;
    Ok(())
}
