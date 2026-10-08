//! Creative flight drill on a strict server-authoritative BDS where the bot is operator: switches to creative,
//! takes off, rises, flies forward, sprints, strafes, descends to land, then drops out of a flight in the air.
//! Prints the server corrections per phase (docs/testing.md "Movement physics").
//! `cargo run -p acacia-bot --example flight -- <server> <name|@account> [rounds]`
//! `BEDROCK_RECORD=<file>` records a movement trace for the `replay` example.
mod support;

use acacia_bot::movement::Controls;
use acacia_bot::Bot;
use support::{corrections, Error};

struct Phase {
    name: &'static str,
    ticks: u32,
    controls: Controls,
}

const fn phase(name: &'static str, ticks: u32, controls: Controls) -> Phase {
    Phase { name, ticks, controls }
}

/// Facing +X, flying, nothing pressed.
const HOVER: Controls = Controls {
    forward: 0.0, strafe: 0.0, jump: false, sneak: false, sprint: false, glide: false, fly: true, yaw: -90.0, pitch: 0.0,
};

const PHASES: &[Phase] = &[
    phase("takeoff", 20, Controls { jump: true, ..HOVER }),
    phase("hover", 20, HOVER),
    phase("forward", 20, Controls { forward: 1.0, ..HOVER }),
    phase("brake", 20, HOVER),
    phase("sprint", 15, Controls { forward: 1.0, sprint: true, yaw: 90.0, ..HOVER }),
    phase("brake2", 20, Controls { yaw: 90.0, ..HOVER }),
    phase("diagonal-up", 15, Controls { forward: 1.0, strafe: 1.0, jump: true, yaw: 45.0, ..HOVER }),
    phase("strafe-down", 10, Controls { strafe: -1.0, sneak: true, yaw: 45.0, ..HOVER }),
    phase("up-down", 10, Controls { jump: true, sneak: true, ..HOVER }),
    phase("descend", 40, Controls { sneak: true, ..HOVER }),
    phase("landed", 20, HOVER),
    phase("rise", 12, Controls { jump: true, ..HOVER }),
    phase("drop", 40, Controls { fly: false, ..HOVER }),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Flyer".into());
    let rounds: u32 = args.next().and_then(|r| r.parse().ok()).unwrap_or(1);
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.run(&bot, "/gamemode creative @s");
    let mut waited = 0;
    while waited < 200 && !bot.state().player.may_fly() {
        bot.wait_ticks(1).await?;
        waited += 1;
    }
    println!("{:?} after {waited} ticks: {:?}", bot.state().player.game_mode, bot.state().player.abilities);
    let mut total = 0;
    for round in 0..rounds {
        pad.build(&mut bot, &[], 10).await?;
        pad.teleport(&mut bot, [0.5, 0.0, 0.5], -90.0).await?;
        bot.trace_mark(&format!("flight {round}"));
        for p in PHASES {
            let before = corrections(&bot);
            if let Some(c) = bot.controls() {
                *c = p.controls;
            }
            bot.wait_ticks(p.ticks).await?;
            let n = corrections(&bot) - before;
            total += n;
            println!("{:12} corrections {n:3}  flying {} server {}  feet {}", p.name, flying(&bot), bot.state().player.abilities.flying, feet(&bot));
        }
    }
    println!("total corrections {total}");
    pad.run(&bot, "/gamemode survival @s");
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

fn flying(bot: &Bot) -> bool {
    bot.movement().is_some_and(|m| m.flying())
}

fn feet(bot: &Bot) -> String {
    bot.movement().and_then(|m| m.position()).map_or("?".into(), |[x, y, z]| format!("{x:.3} {y:.3} {z:.3}"))
}
