//! Java bot: opens the chest at `<x> <y> <z>`, puts hotbar slot 0 in its slot 1, takes everything out, closes it,
//! then reopens it to print what the server holds.
//! `cargo run -p acacia-bot --features java --example java_chest -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::state::{GameState, ItemStack};
use acacia_bot::{Bot, BotConfig};

fn names<'a>(state: &GameState, stacks: impl Iterator<Item = (usize, &'a ItemStack)>) -> Vec<String> {
    stacks.filter(|(_, s)| !s.is_empty()).map(|(i, s)| format!("{i}:{}x{}", state.items.name(s.network_id).unwrap_or("?"), s.count)).collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_chest <host> <port> <x> <y> <z> [name]".into()) };
    let chest = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Chester", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    let opened = bot.open_container_at(chest).await?;
    println!("opened {:?} at {:?}: {:?}", opened.window_type, opened.position, names(bot.state(), opened.iter()));
    println!("put {:?}", bot.put_in_container(0, 1).await);
    println!("take_all {:?}", bot.take_all().await);
    bot.wait_ticks(10).await?;
    println!("close {:?}", bot.close_container().await);
    println!("inventory {:?}", names(bot.state(), bot.state().inventory.main.iter().enumerate()));
    bot.wait_ticks(20).await?;
    let reopened = bot.open_container_at(chest).await?;
    println!("reopened: {:?}", names(bot.state(), reopened.iter()));
    println!("inventory {:?}", names(bot.state(), bot.state().inventory.main.iter().enumerate()));
    bot.close_container().await?;
    bot.wait_ticks(5).await?;
    bot.disconnect().await;
    Ok(())
}
