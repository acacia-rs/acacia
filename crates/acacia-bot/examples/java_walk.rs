//! Java bot: joins an offline-mode server, waits 3 s, sprints forward for 3 s and prints where it started and ended.
//! `cargo run -p acacia-bot --features java --example java_walk -- <host> <port> [name]`

use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().map_or(Ok(25565), |p| p.parse())?;
    let name = args.next().unwrap_or_else(|| "Walker".into());

    let mut bot = Bot::connect_java_offline(&host, port, &name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    let start = position(&bot);
    println!("start {start:?}");
    if let Some(c) = bot.controls() {
        (c.forward, c.sprint) = (1.0, true);
    }
    bot.wait_ticks(60).await?;
    if let Some(c) = bot.controls() {
        c.stop();
    }
    let end = position(&bot);
    println!("end {end:?} moved {:.3}", (end[0] - start[0]).hypot(end[2] - start[2]));
    bot.disconnect().await;
    Ok(())
}

fn position(bot: &Bot) -> [f64; 3] {
    bot.java().map_or([f64::NAN; 3], |j| j.client().movement.player.pos)
}
