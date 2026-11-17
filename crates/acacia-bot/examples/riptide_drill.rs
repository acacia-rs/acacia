//! Riptide drills on a strict server-authoritative BDS where the bot is operator: each phase gives a trident with
//! Riptide, stands the bot in a pool (or in the rain on the pad), charges it 12 ticks and releases it, then holds
//! still while the server's corrections show the launch (docs/research/riptide-impact-exit.md §1).
//! `cargo run -p acacia-bot --example riptide_drill -- <server> <name> [phases]`; `BEDROCK_RECORD=<file>` records.
mod support;

use support::{corrections, Error};

struct Phase {
    name: &'static str,
    level: u8,
    pitch: f32,
    yaw: f32,
    /// Feet start (pad offsets): in the pool, or on the dry pad with rain.
    start: [f32; 3],
    rain: bool,
}

/// Water 2 deep with its top at the pad's surface: standing on the bottom, the feet are in water and the head out.
const POOL: support::Fill<'static> = ([-6, -2, -6], [6, -1, 6], "water");
const IN_POOL: [f32; 3] = [0.5, -2.0, 0.5];
const DRY: [f32; 3] = [10.5, 0.0, 0.5];
const AFTER: u32 = 40;
/// Pool cells next to the -X wall (pool x from -6): a launch at -X hits it within a tick or two.
const NEAR_WALL: [f32; 3] = [-4.5, -2.0, 0.5];

const PHASES: &[Phase] = &[
    Phase { name: "water_p0", level: 3, pitch: 0.0, yaw: -90.0, start: IN_POOL, rain: false },
    Phase { name: "water_up", level: 3, pitch: -30.0, yaw: -90.0, start: IN_POOL, rain: false },
    Phase { name: "water_steep", level: 3, pitch: -90.0, yaw: 0.0, start: IN_POOL, rain: false },
    Phase { name: "water_l1", level: 1, pitch: -30.0, yaw: 45.0, start: IN_POOL, rain: false },
    Phase { name: "water_down", level: 2, pitch: 30.0, yaw: 137.0, start: IN_POOL, rain: false },
    Phase { name: "rain_ground", level: 3, pitch: -30.0, yaw: -90.0, start: DRY, rain: true },
    Phase { name: "rain_flat", level: 3, pitch: 0.0, yaw: -90.0, start: DRY, rain: true },
    Phase { name: "water_wall", level: 3, pitch: 0.0, yaw: 90.0, start: NEAR_WALL, rain: false },
    Phase { name: "water_steep_l2", level: 2, pitch: -60.0, yaw: 0.0, start: IN_POOL, rain: false },
    Phase { name: "dry_none", level: 3, pitch: 0.0, yaw: -90.0, start: DRY, rain: false },
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(600);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Riptider".into());
    let only: Option<Vec<&str>> = args.get(2).map(|s| s.split(',').collect());
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.run(&bot, "/gamemode survival @s");
    for p in PHASES.iter().filter(|p| only.as_ref().is_none_or(|o| o.contains(&p.name))) {
        pad.build(&mut bot, &[POOL], 20).await?;
        pad.run(&bot, if p.rain { "/weather rain" } else { "/weather clear" });
        for cmd in ["/clear @s", "/give @s trident"] {
            pad.run(&bot, cmd);
            bot.wait_ticks(4).await?;
        }
        bot.select_hotbar(0)?;
        bot.wait_ticks(4).await?;
        pad.run(&bot, &format!("/enchant @s riptide {}", p.level));
        pad.teleport(&mut bot, p.start, p.yaw).await?;
        if let Some(c) = bot.controls() {
            (c.yaw, c.pitch) = (p.yaw, p.pitch);
        }
        bot.wait_ticks(20).await?;
        bot.bedrock_mut().expect("Bedrock bot").trace_mark(p.name);
        let before = corrections(&bot);
        bot.riptide().await?;
        bot.wait_ticks(AFTER).await?;
        println!("{:>12}: {} corrections", p.name, corrections(&bot) - before);
    }
    pad.run(&bot, "/weather clear");
    pad.go_home(&mut bot).await?;
    Ok(())
}
