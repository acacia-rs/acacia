//! Server-position oracle: bot A sprint-hops (`OBS_MODE=walk`: sprint-walks; `mix`: draws diagonal keys, sprint,
//! hops and a mid-walk turn per segment) from `OBS_START=x,z` (pad offsets,
//! default 10.5,0.5) at random yaws while bot B logs every `MovePlayer` the server broadcasts for A to
//! `OBSERVE_LOG`: the server's own position each tick, not just at corrections (docs/testing.md).
//! `cargo run --release -p acacia-bot --example observe -- <server> <nameA> <nameB> [yaws] [ticks]`
mod support;

use std::io::Write;

use acacia_bot::client::proto::Packet;
use acacia_bot::client::proto::packets::MovePlayer;
use acacia_bot::client::{Client, PacketFilter};
use acacia_bot::movement::Controls;
use acacia_bot::{Bot, BotConfig, BotEvent};
use support::{Error, corrections};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run()).await
}

async fn run() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name_a = args.next().unwrap_or_else(|| "ObsA".into());
    let name_b = args.next().unwrap_or_else(|| "ObsB".into());
    let count: u32 = args.next().map_or(Ok(10), |s| s.parse())?;
    let ticks: u32 = args.next().map_or(Ok(20), |s| s.parse())?;
    let start: Vec<f32> = std::env::var("OBS_START").unwrap_or_else(|_| "10.5,0.5".into()).split(',').map(str::parse).collect::<Result<_, _>>()?;
    let jump = std::env::var("OBS_MODE").as_deref() != Ok("walk");
    let (mut a, pad) = support::connect(&server, &name_a).await?;
    pad.build(&mut a, &[], 5).await?;

    let config = BotConfig { subscribe: PacketFilter::none().with(MovePlayer::ID), ..BotConfig::default() };
    let mut b = Bot::connect(Client::builder(&server).offline(&name_b), config).await?;
    let mut log = std::io::BufWriter::new(std::fs::File::create(std::env::var("OBSERVE_LOG").unwrap_or_else(|_| "observe.log".into()))?);
    // Lines: `mp <runtime id> <tick> [x, y, z] <mode> <on_ground>`, eye positions.
    let watcher = tokio::task::spawn_local(async move {
        while let Some(e) = b.next().await {
            match e {
                BotEvent::Packet(p) => {
                    if let Ok(m) = p.decode::<MovePlayer>() {
                        let _ = writeln!(log, "mp {} {} {:?} {:?} {}", m.runtime_id, m.tick, [m.position.x, m.position.y, m.position.z], m.mode, m.on_ground);
                    }
                }
                BotEvent::Disconnected(_) => break,
                _ => {}
            }
        }
        let _ = log.flush();
    });
    a.wait_ticks(40).await?;
    let [x, y, z] = pad.block([2, 0, -10]);
    pad.run(&a, &format!("/tp {name_b} {x}.5 {y} {z}.5"));
    a.wait_ticks(100).await?;
    // The test server's pack builds its scene (a water pool) where a player stands 40 ticks after it joins.
    pad.build(&mut a, &[], 20).await?;

    let mut seed: u32 = 0x1234_5678;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let mix = std::env::var("OBS_MODE").as_deref() == Ok("mix");
    for _ in 0..count {
        let yaw = next() * 360.0 - 180.0;
        pad.teleport(&mut a, [start[0], 0.0, start[1]], yaw).await?;
        a.wait_ticks(5).await?;
        let seg = if mix { Segment::draw(&mut next) } else { Segment { strafe: 0.0, sprint: true, jump, turn: Turn::None } };
        a.trace_mark(&format!("hop {yaw} {seg:?}"));
        let before = corrections(&a);
        for t in 0..ticks {
            let y = yaw + seg.turn.offset(t);
            if let Some(c) = a.controls() {
                *c = Controls { forward: 1.0, strafe: seg.strafe, sprint: seg.sprint, jump: seg.jump, yaw: y, ..Controls::default() };
            }
            a.wait_ticks(1).await?;
        }
        if let Some(c) = a.controls() {
            *c = Controls { yaw, ..Controls::default() };
        }
        a.wait_ticks(15).await?;
        println!("yaw {yaw:12} {seg:?} corrections {}", corrections(&a) - before);
    }
    pad.go_home(&mut a).await?;
    a.disconnect().await;
    // B never ends on its own; dropping its task flushes the log.
    watcher.abort();
    Ok(())
}

/// How the yaw moves during an `OBS_MODE=mix` segment, degrees from its start.
#[derive(Debug, Clone, Copy)]
enum Turn {
    None,
    /// A smooth sweep `amp * sin(t / period)`, as Human's aim easing turns.
    Ease { amp: f32, period: f32 },
    /// A jump of `by` every `every` ticks, as an aim snapping to a target.
    Steps { by: f32, every: u32 },
}

impl Turn {
    fn offset(self, t: u32) -> f32 {
        match self {
            Turn::None => 0.0,
            Turn::Ease { amp, period } => amp * (t as f32 / period).sin(),
            Turn::Steps { by, every } => by * (t / every) as f32,
        }
    }
}

/// One `OBS_MODE=mix` segment: forward held, with a strafe key (diagonal), sprint, hops and a turn drawn per segment.
#[derive(Debug, Clone, Copy)]
struct Segment {
    strafe: f32,
    sprint: bool,
    jump: bool,
    turn: Turn,
}

impl Segment {
    fn draw(next: &mut impl FnMut() -> f32) -> Segment {
        let strafe = [0.0, 1.0, -1.0][(next() * 2.999) as usize];
        let turn = match (next() * 2.999) as usize {
            0 => Turn::None,
            1 => Turn::Ease { amp: 10.0 + next() * 30.0, period: 3.0 + next() * 6.0 },
            _ => Turn::Steps { by: next() * 40.0 - 20.0, every: 3 + (next() * 4.0) as u32 },
        };
        Segment { strafe, sprint: strafe == 0.0 && next() < 0.6, jump: next() < 0.5, turn }
    }
}
