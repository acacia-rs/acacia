//! Java bot: crafts planks and sticks in its own grid, then a wooden pickaxe at the crafting table at `<x> <y> <z>`,
//! printing the inventory after each step. Give it 3 oak logs first.
//! `cargo run -p acacia-bot --features java --example java_craft -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::{Bot, BotConfig};
use acacia_world::neutral::Item;

fn inventory(bot: &Bot) -> Vec<String> {
    let state = bot.state();
    let named = |s: &acacia_bot::state::ItemStack| format!("{}x{}", state.items.name(s.network_id).unwrap_or("?"), s.count);
    state.inventory.main.iter().filter(|s| !s.is_empty()).map(named).collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_craft <host> <port> <x> <y> <z> [name]".into()) };
    let table = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Crafter", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    println!("start {:?}", inventory(&bot));
    for (item, count, at) in [(Item::OakPlanks, 12, None), (Item::Stick, 4, None), (Item::WoodenPickaxe, 1, Some(table))] {
        let made = bot.craft(item, count, at).await;
        bot.wait_ticks(10).await?;
        println!("{item:?}: {made:?} -> {:?}", inventory(&bot));
    }
    bot.disconnect().await;
    Ok(())
}
