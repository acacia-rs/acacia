//! Joins a server (offline), keeps every chunk it is sent as a Java world under `<world>`, and leaves once no
//! chunk has changed for `quiet` seconds. `cargo run --release -p acacia-anvil --example crawl --
//! <server> <world dir> [name] [radius=8] [quiet s=10] [limit s=300]`. `ANVIL_TP="x y z;x y z"` visits points
//! in turn by `/tp` (the bot must be op), saving each area before moving on.
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use acacia_anvil::{AnvilStore, StoreConfig};
use acacia_bot::client::Client;
use acacia_bot::{Bot, BotConfig, BotEvent};
use acacia_world::ChunkChange;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut args = std::env::args().skip(1);
    let server = args
        .next()
        .ok_or("usage: crawl <server> <world dir> [name] [radius] [quiet s] [limit s]")?;
    let world_dir = PathBuf::from(args.next().ok_or("missing world dir")?);
    let name = args.next().unwrap_or_else(|| "Anvil".into());
    let radius: i32 = args.next().map_or(Ok(8), |s| s.parse())?;
    let quiet = Duration::from_secs(args.next().map_or(Ok(10), |s| s.parse())?);
    let limit = Duration::from_secs(args.next().map_or(Ok(300), |s| s.parse())?);
    let mut stops: Vec<String> = std::env::var("ANVIL_TP")
        .map(|v| v.split(';').map(str::to_owned).collect())
        .unwrap_or_default();
    stops.reverse();

    let builder = Client::builder(&server)
        .offline(&name)
        .chunk_radius(radius)
        .login_timeout(limit);
    let mut bot = Bot::connect(
        builder,
        BotConfig {
            physics: true,
            ..BotConfig::default()
        },
    )
    .await?;
    let start = Instant::now();
    let (mut store, mut changes, mut pending) = (None, None, HashSet::new());
    let (mut last_change, mut converted) = (Instant::now(), 0usize);
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let outcome = loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(reason)) => break format!("disconnected {reason:?}"),
                None => break "closed".to_owned(),
                Some(_) => {}
            },
            _ = tick.tick() => {
                let Some(view) = bot.world().and_then(|w| w.view()) else { continue };
                let world = view.world().clone();
                if changes.is_none() {
                    changes = Some(world.subscribe());
                    pending.extend(world.chunk_positions());
                    let cfg = StoreConfig {
                        region_dir: acacia_anvil::region_dir(&world_dir, world.dimension()),
                        has_sky: world.dimension().sky,
                        max_wait: quiet,
                        region_interval: Duration::from_secs(2),
                        memory_limit: 1 << 30,
                        threads: 0,
                    };
                    store = Some(AnvilStore::new(world.registry().clone(), cfg)?);
                }
                for change in changes.as_ref().unwrap().try_iter() {
                    pending.insert(match change {
                        ChunkChange::Column { x, z } | ChunkChange::Section { x, z, .. } => (x, z),
                        ChunkChange::Block { x, z, .. } => (x >> 4, z >> 4),
                    });
                }
                let store = store.as_mut().unwrap();
                if !pending.is_empty() {
                    last_change = Instant::now();
                }
                let block_entities = &bot.state().block_entities;
                for (x, z) in pending.drain() {
                    let Some(chunk) = world.get(x, z) else { continue };
                    let bes = block_entities.iter().filter(|(p, _)| p[0] >> 4 == x && p[2] >> 4 == z).map(|(p, n)| (*p, &n.value));
                    store.update(&chunk.read(), bes);
                    converted += 1;
                }
                let stats = store.flush(false)?;
                if stats.columns > 0 {
                    println!("{:>5.1}s wrote {} columns in {} regions, {} waiting, {} held ({} MB)", start.elapsed().as_secs_f32(), stats.columns, stats.regions, stats.waiting, store.columns(), store.heap_bytes() >> 20);
                }
                if last_change.elapsed() >= quiet {
                    match stops.pop() {
                        Some(stop) => {
                            store.flush(true)?;
                            bot.client().command(&format!("/tp @s {stop}"));
                            last_change = Instant::now();
                        }
                        None => break "quiet".to_owned(),
                    }
                }
                if start.elapsed() >= limit {
                    break "timeout".to_owned();
                }
            }
        }
    };
    if let Some(store) = store.as_mut() {
        store.flush(true)?;
    }
    println!(
        "crawl {outcome}: {converted} conversions in {:.1}s",
        start.elapsed().as_secs_f32()
    );
    // ANVIL_PROBE="x,y,z;…": the Bedrock states the bot holds there, for chasing a diff-render cell
    if let (Ok(probe), Some(view)) = (
        std::env::var("ANVIL_PROBE"),
        bot.world().and_then(|w| w.view()),
    ) {
        let reg = view.world().registry();
        for p in probe.split(';') {
            let v: Vec<i32> = p.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            let [x, y, z] = v[..] else { continue };
            let state = |id: u32| {
                reg.get(id).map_or(format!("#{id}"), |s| {
                    format!("{}[{}]", s.name, s.properties)
                })
            };
            match view.chunk(x >> 4, z >> 4) {
                Some(c) => {
                    let c = c.read();
                    println!(
                        "probe {p}: {} / {}",
                        state(c.block(x, y, z)),
                        state(c.liquid(x, y, z))
                    );
                }
                None => println!("probe {p}: no chunk"),
            }
        }
    }
    bot.client().close();
    Ok(())
}
