//! Entity collision and push drills on a strict server-authoritative BDS where the bot is operator: each phase
//! rebuilds the pad, summons one entity, places the bot and holds one control pattern, then reports the server
//! corrections of that phase. Settles which entities BDS makes solid to a player and how mobs push one.
//! `cargo run -p acacia-bot --example entity_drill -- <server> <name> [phases]`
//! `RUST_LOG=acacia_bot::movement=debug` prints each correction and knockback.
//! `DRILL_OBSERVE=<name>` adds a spectator that logs the server's `MovePlayer` broadcasts of the bot (`mp <tick> [eye]`)
//! beside the bot's own `us <tick> [pos] [vel]` and each entity's `e <kind> [pos]` to `DRILL_LOG` (default drill.log).
mod support;

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use acacia_bot::client::proto::Packet;
use acacia_bot::client::proto::packets::MovePlayer;
use acacia_bot::client::{Client, PacketFilter};
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig, BotEvent};
use support::{Error, Pad, corrections};

type Log = Rc<RefCell<std::io::BufWriter<std::fs::File>>>;

struct Phase {
    name: &'static str,
    /// Summoned entity and where (pad offsets, feet).
    entity: &'static str,
    at: [f32; 3],
    /// Console commands run after the summon (`@e` targets the summoned entity's kind).
    setup: &'static [&'static str],
    /// Bot start (pad offsets, feet) and yaw.
    start: [f32; 3],
    yaw: f32,
    /// Ticks idle, then ticks walking forward.
    idle: u32,
    walk: u32,
}

const STILL: &[&str] = &["/effect @e[type=!player] slowness 1000 255 true"];

const PHASES: &[Phase] = &[
    Phase { name: "boat_walk", entity: "boat", at: [4.5, 0.0, 0.5], setup: &[], start: [0.5, 0.0, 0.5], yaw: -90.0, idle: 10, walk: 40 },
    Phase { name: "boat_stand", entity: "boat", at: [4.5, 0.0, 0.5], setup: &[], start: [4.5, 1.5, 0.5], yaw: -90.0, idle: 30, walk: 30 },
    Phase { name: "boat_offaxis", entity: "boat", at: [4.5, 0.0, 1.5], setup: &[], start: [0.5, 0.0, 0.5], yaw: -70.0, idle: 10, walk: 40 },
    Phase { name: "boat_side", entity: "boat", at: [3.0, 0.0, 1.2], setup: &[], start: [0.5, 0.0, 0.5], yaw: -90.0, idle: 10, walk: 40 },
    Phase { name: "minecart_walk", entity: "minecart", at: [4.5, 0.0, 0.5], setup: &[], start: [0.5, 0.0, 0.5], yaw: -90.0, idle: 10, walk: 40 },
    Phase { name: "minecart_stand", entity: "minecart", at: [4.5, 0.0, 0.5], setup: &[], start: [4.5, 1.5, 0.5], yaw: -90.0, idle: 30, walk: 30 },
    Phase { name: "cow_overlap", entity: "cow", at: [2.3, 0.0, 0.5], setup: STILL, start: [2.9, 0.0, 0.7], yaw: -90.0, idle: 40, walk: 0 },
    Phase { name: "cow_walk", entity: "cow", at: [4.5, 0.0, 0.5], setup: STILL, start: [0.5, 0.0, 0.5], yaw: -90.0, idle: 10, walk: 40 },
    Phase { name: "cow_offaxis", entity: "cow", at: [4.5, 0.0, 0.8], setup: STILL, start: [0.5, 0.0, 0.5], yaw: -90.0, idle: 10, walk: 40 },
];

/// Connects the spectator that logs every `MovePlayer` it is sent.
async fn observer(server: &str, name: &str, log: Log) -> Result<tokio::task::JoinHandle<()>, Error> {
    let config = BotConfig { subscribe: PacketFilter::none().with(MovePlayer::ID), ..BotConfig::default() };
    let mut b = Bot::connect(Client::builder(server).offline(name), config).await?;
    Ok(tokio::task::spawn_local(async move {
        while let Some(e) = b.next().await {
            match e {
                BotEvent::Packet(p) => {
                    if let Ok(m) = p.decode::<MovePlayer>() {
                        let _ = writeln!(log.borrow_mut(), "mp {} {:?}", m.tick, [m.position.x, m.position.y, m.position.z]);
                    }
                }
                BotEvent::Disconnected(_) => break,
                _ => {}
            }
        }
    }))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run()).await
}

async fn run() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(900);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19188".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Pusher".into());
    let only: Option<Vec<&str>> = args.get(2).map(|s| s.split(',').collect());
    let config = BotConfig { trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let (mut bot, pad) = support::connect_config(&server, &name, config).await?;
    bot.client().command("/gamemode survival @s");
    let log: Log = Rc::new(RefCell::new(std::io::BufWriter::new(std::fs::File::create(
        std::env::var("DRILL_LOG").unwrap_or_else(|_| "drill.log".into()),
    )?)));
    let watcher = match std::env::var("DRILL_OBSERVE") {
        Ok(obs) => {
            let w = observer(&server, &obs, log.clone()).await?;
            bot.wait_ticks(60).await?;
            bot.client().command(&format!("/gamemode spectator {obs}"));
            Some((obs, w))
        }
        Err(_) => None,
    };
    let mut total = 0;
    for p in PHASES.iter().filter(|p| only.as_ref().is_none_or(|o| o.contains(&p.name))) {
        let n = drill(&mut bot, &pad, p, watcher.as_ref().map(|w| w.0.as_str()), &log).await?;
        println!("{:>16}: {n} corrections", p.name);
        total += n;
    }
    println!("total: {total} corrections");
    if let Some((_, w)) = watcher {
        w.abort();
    }
    log.borrow_mut().flush()?;
    pad.go_home(&mut bot).await?;
    Ok(())
}

async fn drill(bot: &mut Bot, pad: &Pad, p: &Phase, observer: Option<&str>, log: &Log) -> Result<u32, Error> {
    pad.build(bot, &[], 4).await?;
    pad.run(bot, "/kill @e[type=!player]");
    bot.wait_ticks(4).await?;
    let [x, y, z] = pad.block([0, 0, 0]).map(|v| v as f32);
    let at = [x + p.at[0], y + p.at[1], z + p.at[2]];
    pad.run(bot, &format!("/summon {} {} {} {}", p.entity, at[0], at[1], at[2]));
    bot.wait_ticks(4).await?;
    for cmd in p.setup {
        pad.run(bot, cmd);
        bot.wait_ticks(2).await?;
    }
    if let Some(obs) = observer {
        pad.run(bot, &format!("/tp {obs} {} {} {}", at[0], at[1] + 3.0, at[2] + 3.0));
    }
    bot.wait_ticks(20).await?;
    pad.teleport(bot, p.start, p.yaw).await?;
    let _ = writeln!(log.borrow_mut(), "phase {}", p.name);
    for e in bot.state().entities.iter().filter(|e| !e.is_player()) {
        let m = &e.metadata;
        let _ = writeln!(log.borrow_mut(), "kind {} box {:?} flags {:?} {:?}", e.kind, m.bounding_box(), m.flags(), m.flags_extended());
    }
    let before = corrections(bot);
    for t in 0..p.idle + p.walk {
        if let Some(c) = bot.controls() {
            (c.forward, c.yaw, c.pitch) = (if t < p.idle { 0.0 } else { 1.0 }, p.yaw, 0.0);
        }
        bot.wait_ticks(1).await?;
        if let Some(st) = bot.movement().and_then(|m| m.state()) {
            let _ = writeln!(log.borrow_mut(), "us {t} {:?} {:?} ground {}", st.pos, st.vel, st.on_ground);
        }
        for e in bot.state().entities.iter().filter(|e| !e.is_player()) {
            let f = e.feet();
            let _ = writeln!(log.borrow_mut(), "e {} {:?}", e.kind, [f.x, f.y, f.z]);
        }
    }
    if let Some(c) = bot.controls() {
        c.forward = 0.0;
    }
    bot.wait_ticks(10).await?;
    Ok(corrections(bot) - before)
}
