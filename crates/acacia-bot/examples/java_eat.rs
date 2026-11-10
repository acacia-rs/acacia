//! Java bot: waits up to 5 s for hunger, then eats the food in its hand while walking forward, so an anticheat sees
//! the using-item slowdown. Prints health, the food bar and the food left. On a peaceful server hand it something
//! always edible (a golden apple).
//! `cargo run -p acacia-bot --features java --example java_eat -- <host> <port> [name]`

use std::time::Duration;

use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_eat <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Eater", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    let started = tokio::time::Instant::now();
    while bot.state().player.hunger >= 18.0 && started.elapsed() < Duration::from_secs(5) {
        bot.wait_ticks(10).await?;
    }
    let vitals = |bot: &Bot| {
        let p = &bot.state().player;
        (p.health, p.hunger, p.saturation, bot.state().inventory.held().count)
    };
    println!("before (health, food, saturation, held) {:?}", vitals(&bot));
    if let Some(controls) = bot.controls() {
        controls.forward = 1.0;
    }
    let eaten = bot.consume().await;
    if let Some(controls) = bot.controls() {
        controls.forward = 0.0;
    }
    bot.wait_ticks(20).await?;
    println!("consume {eaten:?}; after {:?}", vitals(&bot));
    bot.disconnect().await;
    Ok(())
}
