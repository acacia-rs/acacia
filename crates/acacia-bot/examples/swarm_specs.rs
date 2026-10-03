//! Runs the bots listed in a JSON file (an array of `BotSpec`s, the shape a coordinator would send)
//! and prints swarm events as JSON lines. Online logins read tokens from ./.tokens.
//! `cargo run -p acacia-bot --example swarm_specs --<specs.json> [seconds]`
//! `[{"id":"a1","login":{"kind":"offline","name":"Afk1"},"target":{"kind":"server","address":"127.0.0.1:19132"},"state":null}]`
use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::FileTokenCache;
use acacia_bot::swarm::{BotSpec, Swarm};
use acacia_bot::Bot;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: swarm_specs <specs.json> [seconds]")?;
    let secs: u64 = args.next().map_or(60, |s| s.parse().expect("seconds"));
    let specs: Vec<BotSpec<()>> = serde_json::from_slice(&std::fs::read(path)?)?;

    let swarm = Swarm::builder()
        .token_cache(Arc::new(FileTokenCache::new(".tokens")?))
        .client(|_, b| b.blob_cache_dir(".blobs"))
        .start(async |bot: &mut Bot, _: &mut ()| while bot.next().await.is_some() {})?;
    let mut events = swarm.events();
    for spec in specs {
        swarm.add(spec)?;
    }

    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            Ok(event) = events.recv() => println!("{}", serde_json::to_string(&event)?),
            _ = &mut deadline => break,
        }
    }
    println!("{}", serde_json::to_string(&swarm.snapshot())?);
    swarm.shutdown().await;
    Ok(())
}
