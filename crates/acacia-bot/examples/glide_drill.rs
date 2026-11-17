//! Elytra glide drills on a strict server-authoritative BDS where the bot is operator: each phase teleports the bot
//! 60 blocks up, opens the elytra and holds one control pattern (axis-aligned or off-axis, plain, climbing, boosted,
//! turning), then reports the server corrections of that phase. Pins down where off-axis glides disagree with BDS.
//! `cargo run -p acacia-bot --example glide_drill -- <server> <name|@account> [phases]`
//! `RUST_LOG=acacia_bot::movement=debug` prints each correction.
//! `GLIDE_OBSERVE=<name>` adds a spectator bot mid-path that logs the server's `MovePlayer` broadcasts of the glider
//! (`mp <tick> [x, y, z]`, eye height) beside the glider's own `us [pos] [vel]` after each tick to `GLIDE_LOG`.
mod support;

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use acacia_bot::client::proto::Packet;
use acacia_bot::client::proto::packets::MovePlayer;
use acacia_bot::client::{Client, PacketFilter};
use acacia_bot::{Bot, BotConfig, BotEvent};
use support::{corrections, Error};

type Log = Rc<RefCell<std::io::BufWriter<std::fs::File>>>;

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

struct Phase {
    name: &'static str,
    yaw: f32,
    pitch: f32,
    /// Degrees of yaw added per tick.
    turn: f32,
    rocket: bool,
    ticks: u32,
}

const fn phase(name: &'static str, yaw: f32, pitch: f32, turn: f32, rocket: bool, ticks: u32) -> Phase {
    Phase { name, yaw, pitch, turn, rocket, ticks }
}

const PHASES: &[Phase] = &[
    phase("axis", -90.0, 10.0, 0.0, false, 60),
    phase("offaxis", -50.0, 10.0, 0.0, false, 60),
    phase("offaxis_climb", -50.0, -20.0, 0.0, false, 40),
    phase("boost_axis", -90.0, -10.0, 0.0, true, 40),
    phase("boost_offaxis", -50.0, -10.0, 0.0, true, 40),
    phase("turn", -90.0, 5.0, 1.5, false, 40),
    phase("offaxis_nokeys", -50.0, 10.0, 0.0, false, 60),
    phase("offaxis_p0", -50.0, 0.0, 0.0, false, 60),
    phase("offaxis_45", -45.0, 10.0, 0.0, false, 60),
    phase("offaxis_10", -80.0, 10.0, 0.0, false, 60),
    phase("axis_strafe", -90.0, 10.0, 0.0, false, 60),
    phase("axis_back", -90.0, 10.0, 0.0, false, 60),
    phase("offaxis_back", -50.0, 10.0, 0.0, false, 60),
    phase("land", 0.0, 20.0, 0.0, false, 200),
    phase("land_offaxis", 30.0, 20.0, 0.0, false, 200),
    phase("land_water", 0.0, 20.0, 0.0, false, 200),
    phase("land_boost", 0.0, 25.0, 0.0, true, 200),
    // An elytra 3 (2) uses from breaking (432 durability): the glide wears it out mid-air.
    phase("worn_out", -90.0, 5.0, 0.0, false, 100),
    phase("worn_out2", -90.0, 5.0, 0.0, false, 100),
];

/// A `worn_out` phase's elytra damage.
fn worn_damage(name: &str) -> Option<u32> {
    match name {
        "worn_out" => Some(429),
        "worn_out2" => Some(430),
        _ => None,
    }
}

/// Landing phases glide onto a strip this high over home, from `LAUNCH` above it.
const STRIP_UP: i32 = 30;
const LAUNCH: i32 = 10;
const STRIP_LEN: i32 = 44;
/// Walking ticks recorded after a landing.
const AFTER_LANDING: u32 = 20;

/// A landing phase's strip: stone, or a 3-deep pool in a stone basin for `land_water`; (south-facing).
fn strip_commands(name: &str, [x, y, z]: [i32; 3]) -> Vec<String> {
    let y = y + STRIP_UP;
    let (x0, x1, z1) = (x - 4 - STRIP_LEN / 2, x + 4 + STRIP_LEN / 2, z + STRIP_LEN);
    let mut cmds = vec![format!("/fill {x0} {} {z} {x1} {} {z1} air", y + 1, y + LAUNCH + 3)];
    if name == "land_water" {
        cmds.push(format!("/fill {x0} {} {z} {x1} {y} {z1} stone", y - 3));
        cmds.push(format!("/fill {} {} {} {} {y} {} water", x0 + 1, y - 2, z + 1, x1 - 1, z1 - 1));
    } else {
        cmds.push(format!("/fill {x0} {y} {z} {x1} {y} {z1} stone"));
    }
    cmds
}

/// (forward, strafe) a phase holds, by its name's suffix.
fn keys(name: &str) -> (f32, f32) {
    match name.rsplit('_').next() {
        Some("nokeys") => (0.0, 0.0),
        Some("strafe") => (0.0, 1.0),
        Some("back") => (-1.0, 0.0),
        _ => (1.0, 0.0),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run()).await
}

async fn run() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(600);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19184".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Glider".into());
    let only: Option<Vec<&str>> = args.get(2).map(|s| s.split(',').collect());
    let (mut bot, _pad) = support::connect(&server, &name).await?;
    for cmd in ["/gamemode survival @s", "/clear @s", "/give @s elytra", "/give @s firework_rocket 16"] {
        bot.command(cmd);
        bot.wait_ticks(4).await?;
    }
    bot.wait_ticks(20).await?;
    bot.equip_elytra().await?;
    bot.hold_fireworks().await?;
    let home = bot.block_position().ok_or("no position")?;
    let log: Log = Rc::new(RefCell::new(std::io::BufWriter::new(std::fs::File::create(
        std::env::var("GLIDE_LOG").unwrap_or_else(|_| "glide.log".into()),
    )?)));
    let watcher = match std::env::var("GLIDE_OBSERVE") {
        Ok(obs) => {
            let w = observer(&server, &obs, log.clone()).await?;
            bot.wait_ticks(60).await?;
            bot.command(&format!("/gamemode spectator {obs}"));
            Some((obs, w))
        }
        Err(_) => None,
    };
    for p in PHASES.iter().filter(|p| only.as_ref().is_none_or(|o| o.contains(&p.name))) {
        if let Some((obs, _)) = &watcher {
            // Mid-path and close, so the server sends every tick's move (it paces by watcher distance).
            let (s, c) = (p.yaw.to_radians().sin(), p.yaw.to_radians().cos());
            bot.command(&format!("/tp {obs} {} {} {}", home[0] as f32 - 25.0 * s, home[1] + 40, home[2] as f32 + 25.0 * c));
        }
        if bot.is_gliding() {
            bot.stop_gliding().await?;
            // Fall a while so the stop tick's correction, if any, is not lost under the next teleport.
            bot.wait_ticks(10).await?;
        }
        let landing = p.name.starts_with("land");
        if let Some(damage) = worn_damage(p.name) {
            // A spent elytra drops the bot from the glide height: survive the landing.
            for cmd in [format!("/replaceitem entity @s slot.armor.chest 0 elytra 1 {damage}"), "/effect @s resistance 30 255 true".into()] {
                bot.command(&cmd);
                bot.wait_ticks(4).await?;
            }
        }
        if landing {
            for cmd in strip_commands(p.name, home) {
                bot.command(&cmd);
                bot.wait_ticks(4).await?;
            }
            let top = home[1] + STRIP_UP + 1 + LAUNCH;
            bot.command(&format!("/tp @s {}.5 {top} {}.5 {} 0", home[0], home[2], p.yaw));
        } else {
            bot.command(&format!("/tp @s {} {} {} {} 0", home[0], home[1] + 60, home[2], p.yaw));
        }
        bot.wait_ticks(20).await?;
        bot.start_gliding().await?;
        let _ = writeln!(log.borrow_mut(), "phase {}", p.name);
        bot.bedrock_mut().expect("Bedrock bot").trace_mark(p.name);
        let before = corrections(&bot);
        let (mut yaw, pitch) = (p.yaw, p.pitch);
        if let Some(c) = bot.controls() {
            ((c.forward, c.strafe), c.yaw, c.pitch) = (keys(p.name), yaw, pitch);
        }
        if p.rocket {
            bot.boost_with_firework().await?;
        }
        let mut landed_for = None;
        for _ in 0..p.ticks {
            if landing {
                landed_for = landed_for.map(|n: u32| n + 1).or((!bot.is_gliding()).then_some(0));
                if landed_for == Some(1) {
                    let _ = writeln!(log.borrow_mut(), "landed");
                }
                if landed_for >= Some(AFTER_LANDING) {
                    break;
                }
            }
            yaw += p.turn;
            if let Some(c) = bot.controls() {
                c.yaw = yaw;
            }
            bot.wait_ticks(1).await?;
            if let Some(st) = bot.bedrock().and_then(|b| b.movement()).and_then(|m| m.state()) {
                let _ = writeln!(log.borrow_mut(), "us {:?} {:?}", st.pos, st.vel);
            }
            if worn_damage(p.name).is_some() {
                let damage = bot.state().inventory.armor.get(1).map(|s| s.damage());
                let _ = writeln!(log.borrow_mut(), "elytra damage {damage:?} gliding {}", bot.is_gliding());
            }
        }
        println!("{:>14}: {} corrections; gliding at the end: {}", p.name, corrections(&bot) - before, bot.is_gliding());
        // A teleport while walking moves on the tick BDS otherwise holds (glide3 land_offaxis); keep setups idle.
        if let Some(c) = bot.controls() {
            (c.forward, c.strafe) = (0.0, 0.0);
        }
    }
    if let Some((_, w)) = watcher {
        w.abort();
    }
    log.borrow_mut().flush()?;
    Ok(())
}
