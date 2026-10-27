//! A swarm for profiling busy worlds (tools/swarm-load.sh): `bots` offline bots on one server, the
//! first `wanderers` of them walking with physics between random spots near spawn, the rest
//! standing. Every bot tracks entities, so each sees the others. Prints swarm events as JSON lines.
//! `cargo run --release -p acacia-bot --example swarm_load -- <server> <bots> <wanderers> [seconds] [shards]`
//! `JOIN_MS` replaces the swarm's 500 ms join spacing.
use std::time::Duration;

use acacia_bot::pathfind::{Goal, GotoOpts, NavStatus, Navigator};
use acacia_bot::state::Trackers;
use acacia_bot::swarm::{BotId, BotSpec, Login, Swarm, Target};
use acacia_bot::{ActionError, Bot, BotConfig};

/// Half the side of the square wanderers pick their goals in, around where they spawned.
const ROAM: i32 = 40;
/// Ticks a goal may take before another is picked (a blocked or unreachable one).
const GIVE_UP: u32 = 600;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
enum Role {
    Stand,
    Wander,
}

/// xorshift64*: deterministic per bot, no dependency.
fn next(seed: &mut u64) -> i32 {
    *seed ^= *seed >> 12;
    *seed ^= *seed << 25;
    *seed ^= *seed >> 27;
    (seed.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as i32
}

async fn wander(bot: &mut Bot, mut seed: u64) -> Result<(), ActionError> {
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    bot.wait_ticks(40).await?;
    let [x0, _, z0] = bot.block_position().ok_or(ActionError::Disconnected)?;
    loop {
        let goal = Goal::XZ(x0 + next(&mut seed) % ROAM, z0 + next(&mut seed) % ROAM);
        let mut nav = Navigator::new(goal, GotoOpts::default());
        for _ in 0..GIVE_UP {
            match nav.tick(bot).await {
                Ok(NavStatus::Arrived) => break,
                Ok(_) => {}
                Err(ActionError::Disconnected) => return Err(ActionError::Disconnected),
                Err(_) => {
                    bot.wait_ticks(20).await?;
                    break;
                }
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [server, bots, wanderers, rest @ ..] = args.as_slice() else {
        return Err("usage: swarm_load <server> <bots> <wanderers> [seconds] [shards]".into());
    };
    let (bots, wanderers): (usize, usize) = (bots.parse()?, wanderers.parse()?);
    let secs: u64 = rest.first().map_or(Ok(120), |s| s.parse())?;

    let mut builder = Swarm::builder().bot_config(|spec: &BotSpec<Role>| BotConfig {
        physics: matches!(spec.state, Role::Wander),
        trackers: Trackers { entities: true, ..Trackers::default() },
        ..BotConfig::default()
    });
    if let Some(ms) = std::env::var("JOIN_MS").ok().map(|s| s.parse()).transpose()? {
        builder = builder.join_spacing(Duration::from_millis(ms), Duration::from_millis(ms / 2));
    }
    if let Some(shards) = rest.get(1) {
        builder = builder.shards(shards.parse()?);
    }
    let swarm = builder.start(async |bot: &mut Bot, role: &mut Role| match role {
        Role::Stand => while bot.next().await.is_some() {},
        Role::Wander => {
            let seed = bot.client().display_name().bytes().fold(0x9e37_79b9_7f4a_7c15u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            let _ = wander(bot, seed).await;
        }
    })?;
    let mut events = swarm.events();
    for i in 0..bots {
        let state = if i < wanderers { Role::Wander } else { Role::Stand };
        let login = Login::Offline { name: format!("Load{i}") };
        swarm.add(BotSpec { id: BotId(format!("b{i}")), login, target: Target::Server { address: server.clone() }, proxy: None, state })?;
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
