//! Scripted movement drills on a server where the bot is operator: exact control sequences on a
//! flat pad (plus whatever blocks a drill needs), reporting server corrections per drill. Used to
//! pin down vanilla movement rules the simulation must follow. The drills are in `drills/land.rs` and
//! `drills/liquid.rs`.
//! `cargo run -p acacia-bot --example drills -- <server> <name|@account> [drills]`
//! `BEDROCK_RECORD=<file>` records a movement trace for the `replay` example.
mod support;
#[path = "drills/land.rs"]
mod land;
#[path = "drills/liquid.rs"]
mod liquid;

use acacia_bot::movement::Controls;
use acacia_bot::Bot;
use support::{corrections, Error};

/// (ticks, forward, sprint, jump, yaw offset from +X in degrees, sneak)
type Step = (u32, f32, bool, bool, f32, bool);
type Fill = support::Fill<'static>;

struct Drill {
    name: &'static str,
    fills: &'static [Fill],
    /// Feet start, offset from the origin block's corner.
    start: [f32; 3],
    /// Ticks to wait after building (flowing liquids need time).
    settle: u32,
    steps: &'static [Step],
    /// Look pitch for the whole drill (negative looks up).
    pitch: f32,
}

const FLAT: [f32; 3] = [0.5, 0.0, 0.5];
const STOP: Step = (20, 0.0, false, false, 0.0, false);
const FALL: &[Step] = &[(60, 0.0, false, false, 0.0, false)];
const IDLE: &[Step] = &[(40, 0.0, false, false, 0.0, false)];

const fn drill(name: &'static str, fills: &'static [Fill], steps: &'static [Step]) -> Drill {
    Drill { name, fills, start: FLAT, settle: 30, steps, pitch: 0.0 }
}

/// Commands a drill runs around its steps: before the teleport onto the pad, after it, and when done.
#[derive(Default)]
struct Hooks {
    before: &'static [&'static str],
    after: &'static [&'static str],
    teardown: &'static [&'static str],
}

/// A drill's hooks, by name prefix.
fn hooks(name: &str) -> Hooks {
    let prefix = name.split('_').next().unwrap_or(name);
    match prefix {
        // The startup fire resistance can be gone by now (lev1: lev_lava burned to death).
        "lev" => Hooks {
            before: &["/effect @s fire_resistance 3600 1 true"],
            after: &["/effect @s levitation 30 0 true"],
            teardown: &["/effect @s levitation 0"],
        },
        "lev3" => Hooks { after: &["/effect @s levitation 30 2 true"], teardown: &["/effect @s levitation 0"], ..Hooks::default() },
        _ if name.starts_with("powderboots") => Hooks {
            before: &["/replaceitem entity @s slot.armor.feet 0 leather_boots"],
            teardown: &["/replaceitem entity @s slot.armor.feet 0 air"],
            ..Hooks::default()
        },
        // Hunger 255 drains the food bar in about 80 ticks; the drill idles 120 first.
        "hungrysprint" => Hooks {
            after: &["/effect @s hunger 10 255 true"],
            teardown: &["/effect @s hunger 0", "/effect @s saturation 1 255 true"],
            ..Hooks::default()
        },
        // A block in the head's cell: neither standing nor sneaking fits.
        "crawlenter" => Hooks { after: &["/setblock ~ ~1 ~ stone"], ..Hooks::default() },
        "spectatorwall" => Hooks { before: &["/gamemode spectator @s"], teardown: &["/gamemode survival @s"], ..Hooks::default() },
        _ => Hooks::default(),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let only: Option<Vec<String>> = args.next().map(|l| l.split(',').map(str::to_owned).collect());
    let (mut bot, pad) = support::connect(&server, &name).await?;
    // Magma floors burn.
    pad.run(&bot, "/effect @s fire_resistance 3600 1 true");

    for drill in land::DRILLS.iter().chain(liquid::DRILLS) {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == drill.name)) {
            continue;
        }
        let hooks = hooks(drill.name);
        bot.bedrock_mut().expect("Bedrock bot").trace_mark(&format!("{} build", drill.name));
        pad.build(&mut bot, drill.fills, drill.settle).await?;
        for cmd in hooks.before {
            pad.run(&bot, cmd);
            bot.wait_ticks(4).await?;
        }
        pad.teleport(&mut bot, drill.start, -90.0).await?;
        bot.bedrock_mut().expect("Bedrock bot").trace_mark(drill.name);
        if std::env::var("BEDROCK_COLUMN").is_ok() {
            println!("  column under start: {}", column(&bot));
        }
        for cmd in hooks.after {
            pad.run(&bot, cmd);
        }
        let before = corrections(&bot);
        for &(ticks, forward, sprint, jump, yaw, sneak) in drill.steps {
            if let Some(c) = bot.controls() {
                // Bedrock yaw 0 faces +Z; -90 faces +X.
                *c = Controls { forward, sprint, jump, sneak, yaw: -90.0 + yaw, pitch: drill.pitch, ..Controls::default() };
            }
            bot.wait_ticks(ticks).await?;
        }
        bot.wait_ticks(10).await?;
        for cmd in hooks.teardown {
            pad.run(&bot, cmd);
        }
        println!("{:12} corrections {}", drill.name, corrections(&bot) - before);
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

/// Our view of the blocks from the feet down six cells (debugging stale terrain).
fn column(bot: &Bot) -> String {
    use acacia_world::BlockAccess;
    let (Some([x, y, z]), Some(w)) = (bot.block_position(), bot.bedrock().and_then(|b| b.world())) else { return "?".into() };
    let (Some(view), Some(r)) = (w.view(), w.registry()) else { return "?".into() };
    (0..6)
        .map(|d| r.get(view.block(x, y - d, z)).map_or("?", |s| s.name.trim_start_matches("minecraft:")))
        .collect::<Vec<_>>()
        .join(" ")
}
