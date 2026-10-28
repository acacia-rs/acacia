//! Live entity avoidance (pathfind M4) on a BDS where the bot is operator, at night on normal difficulty: a goto
//! across the pad with `Evade` above it, per scenario and run, reporting the health the bot lost.
//! `zombie`: one zombie placed by the run's seed beside the way (pass: no hit); `creeper`: one lit beside the way
//! (pass: no blast); `skeleton`: one at the pad's edge (pass: at most 2 arrows); `cow`: a slowed cow in a 1-wide
//! corridor (pass: arrives, never `Stuck`).
//! `cargo run -p acacia-bot --example mobs -- <server> <name> <optimal|human> [scenario,..] [runs]`;
//! `MOBS_TRACE=1` prints the bot, the process in control and the field's blasts every 10 ticks.
mod support;

use acacia_bot::pathfind::process::Status;
use acacia_bot::pathfind::{Evade, hazard, Goal, Goto, GotoOpts, MovementProfile, Processes};
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig};
use support::{Error, Fill, Pad, corrections};

const MAX_TICKS: u32 = 900;
const GOAL: [i32; 3] = [22, 0, 0];
const SETUP: &[&str] = &["/difficulty normal", "/time set midnight", "/gamerule dodaylightcycle false", "/gamerule domobspawning false", "/gamemode survival @s"];

struct Scenario {
    name: &'static str,
    fills: &'static [Fill<'static>],
    /// Allowed hits (health drops) for a pass.
    hits: u32,
    summon: fn(&Pad, u64) -> Vec<String>,
}

fn at(pad: &Pad, [x, y, z]: [f32; 3]) -> String {
    let [bx, by, bz] = pad.block([0, 0, 0]).map(|v| v as f32);
    format!("{} {} {}", bx + x, by + y, bz + z)
}

fn spread(seed: u64, n: u64) -> f32 {
    (seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as f32 % n as f32
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "zombie",
        fills: &[],
        hits: 0,
        summon: |pad, seed| {
            let side = if seed % 2 == 0 { 1.0 } else { -1.0 };
            vec![format!("/summon zombie {}", at(pad, [8.5 + spread(seed, 8), 0.0, side * (3.5 + spread(seed >> 3, 6))]))]
        },
    },
    Scenario {
        name: "creeper",
        fills: &[],
        hits: 0,
        summon: |pad, seed| {
            let side = if seed % 2 == 0 { 1.0 } else { -1.0 };
            vec![format!("/summon creeper {} 0 0 minecraft:start_exploding_forced", at(pad, [5.5 + spread(seed, 3), 0.0, side * 3.0]))]
        },
    },
    Scenario { name: "skeleton", fills: &[], hits: 2, summon: |pad, _| vec![format!("/summon skeleton {}", at(pad, [11.5, 0.0, 10.5]))] },
    Scenario {
        name: "cow",
        fills: &[([-1, 0, -1], [20, 1, 1], "stone"), ([0, 0, 0], [20, 1, 0], "air")],
        hits: 0,
        summon: |pad, _| vec![format!("/summon cow {}", at(pad, [10.5, 0.0, 0.5])), "/effect @e[type=cow] slowness 600 255 true".into()],
    },
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run()).await
}

async fn run() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(3600);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19188".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Mobber".into());
    let profile = if args.get(2).is_some_and(|p| p == "human") { MovementProfile::Human } else { MovementProfile::Optimal };
    let only: Option<Vec<&str>> = args.get(3).map(|s| s.split(',').collect());
    let runs: u64 = args.get(4).and_then(|r| r.parse().ok()).unwrap_or(20);
    let config = BotConfig { trackers: Trackers { hazards: true, ..Trackers::default() }, ..BotConfig::default() };
    let (mut bot, pad) = support::connect_config(&server, &name, config).await?;
    for cmd in SETUP {
        pad.run(&bot, cmd);
        bot.wait_ticks(2).await?;
    }
    let mut failed = 0;
    for s in SCENARIOS.iter().filter(|s| only.as_ref().is_none_or(|o| o.contains(&s.name))) {
        let mut passed = 0;
        for seed in 0..runs {
            let r = attempt(&mut bot, &pad, s, profile, seed).await?;
            let ok = r.arrived && r.hits <= s.hits;
            passed += u32::from(ok);
            println!("{:8} {profile:?} seed {seed:2}: {} {} ticks, hits {}, health lost {:.0}, corrections {}, evading {} {:?} goto {}", s.name, if ok { "PASS" } else { "FAIL" }, r.ticks, r.hits, r.lost, r.corrections, r.evading, r.seen, r.goto);
        }
        println!("{:8} {profile:?}: {passed}/{runs} passed", s.name);
        failed += runs as u32 - passed;
    }
    println!("{failed} failed");
    pad.run(&bot, "/kill @e[type=!player]");
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

struct Run {
    arrived: bool,
    ticks: u32,
    hits: u32,
    lost: f32,
    corrections: u32,
    /// Ticks `Evade` was in control.
    evading: u32,
    seen: Seen,
    goto: String,
}

async fn attempt(bot: &mut Bot, pad: &Pad, s: &Scenario, profile: MovementProfile, seed: u64) -> Result<Run, Error> {
    pad.run(bot, "/kill @e[type=!player]");
    pad.build(bot, s.fills, 5).await?;
    for cmd in ["/effect @s instant_health 1 10 true", "/effect @s saturation 1 10 true", "/effect @s clear"] {
        pad.run(bot, cmd);
        bot.wait_ticks(2).await?;
    }
    pad.teleport(bot, [0.5, 0.0, 0.5], -90.0).await?;
    bot.wait_ticks(10).await?;
    for cmd in (s.summon)(pad, seed) {
        pad.run(bot, &cmd);
        bot.wait_ticks(1).await?;
    }
    let mut opts = GotoOpts { seed: Some(seed), give_up_ticks: Some(400), ..GotoOpts::default() };
    opts.search.profile = profile;
    let mut procs = Processes::new();
    let goto = procs.add(Goto::with(Goal::Block(pad.block(GOAL)), opts.clone()), 10);
    let evade = procs.add(Evade::new(opts), 90);
    let (before, start) = (corrections(bot), bot.state().player.health);
    let (mut ticks, mut hits, mut health, mut evading, mut seen) = (0, 0, start, 0, Seen::default());
    while ticks < MAX_TICKS && procs.outcome(goto).is_none() {
        if procs.tick(bot).await? == Status::Idle {
            bot.wait_ticks(1).await?;
        }
        let now = bot.state().player.health;
        hits += u32::from(now < health - 0.01);
        health = now;
        ticks += 1;
        evading += u32::from(procs.arbiter().active() == Some(evade));
        let feet = bot.movement().and_then(|m| m.position()).unwrap_or_default();
        seen.add(&hazard::Threats::near(&bot.state().entities, feet, 16.0), feet);
        let near = |e: &&acacia_bot::state::Entity| !e.is_player() && (e.position.x - feet[0]).hypot(e.position.z - feet[2]) < 16.0;
        seen.kinds.extend(bot.state().entities.iter().filter(near).map(|e| e.kind.clone()));
        if std::env::var_os("MOBS_TRACE").is_some() && ticks % 10 == 0 {
            let blasts = procs.pilot().and_then(|p| p.field()).map(|h| h.blasts.clone());
            println!("  tick {ticks} at {feet:.2?} active {:?} blasts {blasts:.1?} entities {}", procs.arbiter().active(), bot.state().entities.len());
        }
    }
    // Late blasts and arrows still count.
    bot.wait_ticks(40).await?;
    let now = bot.state().player.health;
    hits += u32::from(now < health - 0.01);
    let arrived = matches!(procs.outcome(goto), Some(Ok(())));
    let goto = procs.outcome(goto).map(|r| format!("{r:?}")).unwrap_or_else(|| "running".into());
    Ok(Run { arrived, ticks, hits, lost: start - bot.state().player.health.min(health), corrections: corrections(bot) - before, evading, seen, goto })
}

/// Hostiles tracked within 16 blocks over a run.
#[derive(Debug)]
struct Seen {
    /// Ticks a lit creeper was among them.
    lit: u32,
    /// Closest one came (horizontal blocks), fastest one moved (blocks a tick), shortest one stood (a baby is ~1).
    closest: f32,
    fastest: f32,
    shortest: f32,
    kinds: std::collections::BTreeSet<String>,
}

impl Default for Seen {
    fn default() -> Self {
        Seen { lit: 0, closest: f32::INFINITY, fastest: 0.0, shortest: f32::INFINITY, kinds: Default::default() }
    }
}

impl Seen {
    fn add(&mut self, threats: &hazard::Threats, feet: [f32; 3]) {
        self.lit += u32::from(threats.list.iter().any(|t| t.lit));
        for t in threats.list.iter().filter(|t| t.hostile()) {
            self.closest = self.closest.min((t.feet[0] - feet[0]).hypot(t.feet[2] - feet[2]));
            self.fastest = self.fastest.max(t.motion[0].hypot(t.motion[2]));
            self.shortest = self.shortest.min(t.size.1);
        }
    }
}
