//! Per-bot memory: `bots` standing offline bots in one single-thread process, joined one after another,
//! then `SETTLE` seconds (default 25) of standing before the process prints its memory as one JSON line.
//! `cargo run --release -p acacia-bot --example memory_bench -- <server> <bots>`
//! `PHYSICS=1` runs physics on every bot, `WORLDS=own` gives each bot its own chunk store, `JOIN_MS`
//! (default 100) spaces the joins.
use std::time::{Duration, Instant};

use acacia_bot::client::Client;
use acacia_bot::state::Trackers;
use acacia_bot::world::SharedWorlds;
use acacia_bot::{Bot, BotConfig};
use tokio::sync::mpsc;

type Error = Box<dyn std::error::Error>;

/// Every joiner gets every bot's skin in one PlayerList batch: ~33 MB at 100 bots.
const MAX_BATCH: usize = 128 << 20;

enum Joined {
    Spawned,
    Failed(String),
}

fn env<T: std::str::FromStr>(name: &str, default: T) -> Result<T, Error>
where
    T::Err: std::error::Error + 'static,
{
    std::env::var(name).ok().map_or(Ok(default), |s| s.parse().map_err(Into::into))
}

/// `key: N kB` lines of a /proc file, in kB.
fn proc_kb(path: &str, keys: &[&str]) -> Result<Vec<(String, u64)>, Error> {
    let text = std::fs::read_to_string(path)?;
    Ok(keys
        .iter()
        .filter_map(|key| {
            let line = text.lines().find(|l| l.strip_prefix(key).is_some_and(|r| r.starts_with(':')))?;
            Some((key.to_string(), line.split_whitespace().nth(1)?.parse().ok()?))
        })
        .collect())
}

async fn stand(server: String, i: usize, config: BotConfig, delay: Duration, joined: mpsc::UnboundedSender<Joined>) {
    tokio::time::sleep(delay).await;
    let builder = Client::builder(&server).offline(format!("Mem{i}")).max_batch_bytes(MAX_BATCH);
    match Bot::connect(builder, config).await {
        Ok(mut bot) => {
            let _ = joined.send(Joined::Spawned);
            while bot.next().await.is_some() {}
            eprintln!("Mem{i} disconnected: {:?}", bot.disconnect_reason());
        }
        Err(e) => {
            let _ = joined.send(Joined::Failed(e.to_string()));
        }
    }
}

async fn run(server: String, bots: usize) -> Result<(), Error> {
    let physics = env("PHYSICS", 0u8)? == 1;
    let own_worlds = std::env::var("WORLDS").as_deref() == Ok("own");
    let spacing = Duration::from_millis(env("JOIN_MS", 100u64)?);
    let settle = Duration::from_secs(env("SETTLE", 25u64)?);

    let shared = SharedWorlds::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let start = Instant::now();
    for i in 0..bots {
        let worlds = if own_worlds { SharedWorlds::new() } else { shared.clone() };
        let config = BotConfig { physics, shared_worlds: worlds, trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
        tokio::task::spawn_local(stand(server.clone(), i, config, spacing * i as u32, tx.clone()));
    }
    drop(tx);
    let (mut spawned, mut failed) = (0, 0);
    while spawned + failed < bots {
        match rx.recv().await {
            Some(Joined::Spawned) => spawned += 1,
            Some(Joined::Failed(e)) => {
                eprintln!("join failed: {e}");
                failed += 1;
            }
            None => break,
        }
    }
    let joined_s = start.elapsed().as_secs_f64();
    tokio::time::sleep(settle).await;

    let status = proc_kb("/proc/self/status", &["VmRSS", "VmHWM", "RssAnon", "RssFile"])?;
    let rollup = proc_kb("/proc/self/smaps_rollup", &["Rss", "Pss", "Anonymous", "Private_Dirty"])?;
    let kb = |list: &[(String, u64)]| serde_json::Value::Object(list.iter().map(|(k, v)| (k.clone(), (*v).into())).collect());
    let report = serde_json::json!({
        "client": "acacia", "bots": bots, "spawned": spawned, "failed": failed, "physics": physics,
        "worlds": if own_worlds { "own" } else { "shared" }, "joined_s": joined_s,
        "status_kb": kb(&status), "smaps_rollup_kb": kb(&rollup),
    });
    println!("{report}");
    Ok(())
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [server, bots] = args.as_slice() else {
        return Err("usage: memory_bench <server> <bots>".into());
    };
    let bots: usize = bots.parse()?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    tokio::task::LocalSet::new().block_on(&runtime, run(server.clone(), bots))
}
