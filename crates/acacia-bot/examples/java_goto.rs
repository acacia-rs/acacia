//! Java bot: joins an offline-mode server, waits 3 s, then walks to a block with the shared pathfinder.
//! `cargo run -p acacia-bot --features java --example java_goto -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::pathfind::Goal;
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_goto <host> <port> <x> <y> <z> [name]".into()) };
    let goal = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Walker", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    println!("start {:?}", bot.block_position());
    let started = std::time::Instant::now();
    let result = bot.goto(Goal::Block(goal)).await;
    println!("goto {result:?} after {:.1} s, at {:?}", started.elapsed().as_secs_f32(), bot.block_position());
    bot.disconnect().await;
    Ok(())
}
