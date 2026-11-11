//! The swarm side of tools/bench100.sh, an Endstone-style load benchmark: `bots` idle bots join at
//! once, each teleports to its own grid cell (no two views share a chunk) and stays there. The swarm
//! waits for the views to load (new chunks per player per second below `--rate`, between
//! `--min-settle` and `--max-settle` seconds after the joins), then holds a `--steady` window.
//! Prints JSON lines with wall-clock `t` (unix seconds): `phase` markers, `progress` every 5 s,
//! `gametime` samples, one `view` line per bot and the swarm snapshot.
//! `bench100 <server> <bots> [--key value]...`; keys and defaults in opts.rs.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use acacia_bot::events::Events;
use acacia_bot::swarm::{BotId, BotSpec, Login, Swarm, Target};
use acacia_bot::{Bot, BotConfig, BotEvent};
use acacia_client::proto::Packet;
use acacia_client::proto::packets::{CommandOutput, LevelChunk};
use serde_json::json;

mod opts;

use opts::Opts;

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

#[derive(Default)]
struct View {
    /// When the bot's position first landed in its cell.
    arrived: Option<f64>,
    /// Arrival times of distinct chunks inside the cell's view, in order.
    chunks: Vec<f64>,
    seen: HashSet<(i32, i32)>,
    /// Every LevelChunk, cell or not, and the last position: what went wrong when `chunks` stays 0.
    all_chunks: usize,
    last: [f32; 3],
}

impl View {
    /// Seconds from arrival until `share` of the final chunk set had come.
    fn load_time(&self, share: f64) -> Option<f64> {
        let need = ((self.chunks.len() as f64) * share).ceil() as usize;
        Some(self.chunks.get(need.checked_sub(1)?)? - self.arrived?)
    }
}

#[derive(Clone)]
struct Cell {
    index: usize,
    chunk: (i32, i32),
}

type Views = Arc<Vec<Mutex<View>>>;

async fn run_bot(bot: &mut Bot, cell: &Cell, o: &Opts, views: &Views) {
    let (cx, cz) = cell.chunk;
    let (bx, bz) = (cx * 16 + 8, cz * 16 + 8);
    let tp = o.tp_cmd.as_ref().map(|c| {
        c.replace("{x}", &bx.to_string())
            .replace("{y}", &o.y.to_string())
            .replace("{z}", &bz.to_string())
    });
    let view = &views[cell.index];
    let reach = o.radius + 2;
    let mut next_poll = 0.0;
    // Commands sent before the bot leaves the loading screen (~4 s of the spawn sequence) go
    // unanswered, so the teleport starts after it and repeats until the bot is in its cell.
    let mut next_tp = now() + 5.0;
    while let Some(event) = bot.next().await {
        let t = now();
        let mut v = view.lock().unwrap();
        let p = &bot.state().player.position;
        v.last = [p.x, p.y, p.z];
        if v.arrived.is_none() {
            if (p.x.floor() as i32).div_euclid(16) == cx
                && (p.z.floor() as i32).div_euclid(16) == cz
            {
                v.arrived = Some(t);
            }
        }
        match event {
            BotEvent::Packet(p) if p.id == LevelChunk::ID => {
                let Ok(c) = p.decode::<LevelChunk>() else {
                    continue;
                };
                v.all_chunks += 1;
                if c.dimension == 0
                    && (c.x - cx).abs() <= reach
                    && (c.z - cz).abs() <= reach
                    && v.seen.insert((c.x, c.z))
                {
                    v.chunks.push(t);
                }
            }
            BotEvent::Packet(p) if p.id == CommandOutput::ID => {
                drop(v);
                if let Ok(out) = p.decode::<CommandOutput>() {
                    report_command(t, cell.index, &out);
                }
            }
            BotEvent::Tick => {
                let arrived = v.arrived.is_some();
                drop(v);
                if let Some(tp) = tp.as_ref().filter(|_| !arrived && t >= next_tp) {
                    if let Some(cmd) = &o.setup_cmd {
                        bot.client().command(cmd);
                    }
                    bot.client().command(tp);
                    next_tp = t + 5.0;
                }
                if cell.index == 0 && t >= next_poll {
                    if let Some(cmd) = &o.time_cmd {
                        bot.client().command(cmd);
                    }
                    next_poll = t + o.poll;
                }
            }
            BotEvent::Disconnected(_) => break,
            _ => {}
        }
    }
}

/// A `gametime` line from a `commands.time.query.*` reply; failed commands and teleport replies are
/// printed as they came.
fn report_command(t: f64, bot: usize, out: &CommandOutput) {
    let time = out
        .output
        .iter()
        .find(|m| m.message_id.contains("time.query"));
    if let Some(n) = time.and_then(|m| {
        m.parameters
            .iter()
            .find_map(|p| p.trim().parse::<i64>().ok())
    }) {
        println!("{}", json!({"t": t, "gametime": n}));
    } else if time.is_some()
        || out.success_count == 0
        || out
            .output
            .iter()
            .any(|m| m.message_id.contains("tp") || m.message_id.contains("eleport"))
    {
        println!(
            "{}",
            json!({"t": t, "bot": bot, "command_output": format!("{:?}", out.output)})
        );
    }
}

fn phase(name: &str) -> f64 {
    let t = now();
    println!("{}", json!({"t": t, "phase": name}));
    t
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let o = Arc::new(Opts::parse()?);
    let views: Views = Arc::new((0..o.bots).map(|_| Mutex::new(View::default())).collect());

    let radius = o.radius;
    let mut builder = Swarm::builder()
        .join_spacing(Duration::ZERO, Duration::ZERO)
        .client(move |_, b| b.chunk_radius(radius).max_batch_bytes(128 << 20))
        .bot_config(|_: &BotSpec<Cell>| BotConfig {
            events: Events::TICKS,
            subscribe: [LevelChunk::ID, CommandOutput::ID].into_iter().collect(),
            ..BotConfig::default()
        });
    if let Some(shards) = o.shards {
        builder = builder.shards(shards);
    }
    let (task_o, task_views) = (o.clone(), views.clone());
    let swarm = builder.start(async move |bot: &mut Bot, cell: &mut Cell| {
        run_bot(bot, cell, &task_o, &task_views).await
    })?;
    let mut events = swarm.events();

    let join = phase("join");
    for i in 0..o.bots {
        let chunk = o.cell(i);
        println!(
            "{}",
            json!({"t": now(), "cell": i, "name": format!("Bench{i}"), "x": chunk.0 * 16 + 8, "y": o.y, "z": chunk.1 * 16 + 8})
        );
        let login = Login::Offline {
            name: format!("Bench{i}"),
        };
        swarm.add(BotSpec {
            id: BotId(format!("b{i}")),
            login,
            target: Target::Server {
                address: o.server.clone(),
            },
            proxy: None,
            state: Cell { index: i, chunk },
        })?;
    }

    // (time, chunks counted) every second, for the trailing rate.
    let mut history: Vec<(f64, usize)> = Vec::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut settled = None;
    let mut steady_end = f64::INFINITY;
    loop {
        tokio::select! {
            Ok(event) = events.recv() => println!("{}", serde_json::to_string(&event)?),
            _ = ticker.tick() => {
                let t = now();
                let (arrived, chunks) = views.iter().fold((0, 0), |(a, c), v| {
                    let v = v.lock().unwrap();
                    (a + usize::from(v.arrived.is_some()), c + v.chunks.len())
                });
                history.push((t, chunks));
                let back = history.iter().rev().find(|(t0, _)| t - t0 >= 10.0).copied();
                let rate = back.map(|(t0, c0)| (chunks - c0) as f64 / (t - t0) / o.bots as f64);
                if history.len() % 5 == 0 {
                    println!("{}", json!({"t": t, "progress": {"arrived": arrived, "chunks": chunks, "rate": rate}}));
                }
                let since = t - join;
                if settled.is_none() && (since >= o.max_settle || since >= o.min_settle && arrived == o.bots && rate.is_some_and(|r| r < o.rate)) {
                    settled = Some(phase("steady"));
                    steady_end = t + o.steady;
                }
                if t >= steady_end {
                    phase("end");
                    break;
                }
            }
        }
    }
    for (i, v) in views.iter().enumerate() {
        let v = v.lock().unwrap();
        println!(
            "{}",
            json!({"view": i, "arrived": v.arrived.map(|a| a - join), "chunks": v.chunks.len(), "load90": v.load_time(0.9), "all_chunks": v.all_chunks, "position": v.last})
        );
    }
    println!("{}", serde_json::to_string(&swarm.snapshot())?);
    swarm.shutdown().await;
    Ok(())
}
