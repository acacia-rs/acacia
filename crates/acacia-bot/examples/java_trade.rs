//! Java bot trading with the nearest villager: offer 0 once, then offer 1 three times, printing the offers and the
//! inventory. Give it a farmer selling bread for an emerald (offer 0) and buying 20 wheat (offer 1), 2 emeralds and
//! 64 wheat first.
//! `cargo run -p acacia-bot --features java --example java_trade -- <host> <port> [name]`

use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_trade <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Trader", String::as_str);
    let mut config = BotConfig::default();
    config.trackers.entities = true;
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, config).await?;
    bot.wait_ticks(60).await?;

    let eye = bot.eye_position();
    let apart = |e: &&acacia_bot::state::Entity| {
        let f = e.feet();
        (f.x - eye[0]).hypot(f.z - eye[2])
    };
    let villager = bot.state().entities.iter().filter(|e| e.kind.contains("villager")).min_by(|a, b| apart(a).total_cmp(&apart(b)));
    let kinds: Vec<_> = bot.state().entities.iter().map(|e| e.kind.clone()).collect();
    let villager = villager.map(|e| e.runtime_id).ok_or_else(|| format!("no villager among {kinds:?}"))?;
    println!("trade 0 x1 {:?}", bot.trade(villager, 0, 1).await);
    for o in bot.state().stations.trade.iter().flat_map(|t| &t.offers) {
        println!("  offer {:?} {:?} -> {:?} uses {}/{}", o.buy_a, o.buy_b, o.sell, o.uses, o.max_uses);
    }
    bot.wait_ticks(10).await?;
    println!("trade 1 x3 {:?}", bot.trade(villager, 1, 3).await);
    bot.wait_ticks(10).await?;
    for s in bot.state().inventory.main.iter().filter(|s| !s.is_empty()) {
        println!("  {} x{}", bot.state().items.name(s.network_id).unwrap_or("?"), s.count);
    }
    bot.disconnect().await;
    Ok(())
}
