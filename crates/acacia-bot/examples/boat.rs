//! Boat leg drill on a strict server-authoritative flat BDS where the bot is operator: digs a 6-wide, 2-deep river
//! from the bot's spot (east `a`, north `b`, east `c` blocks), gives a boat, then `boat_to` the far bank; prints the
//! boat's corrections (height only, horizontal), the player's, the ticks and where it ended
//! (docs/pathfind-plan.md "M3").
//! `cargo run -p acacia-bot --example boat -- <server> <name> [a b c]`; `BEDROCK_PROFILE=human` for Human,
//! `KEEP_BOAT=1` leaves the boat, `SUMMON_BOAT=1` boards a summoned one.
mod support;

use acacia_bot::pathfind::boat::BoatOpts;
use acacia_bot::pathfind::MovementProfile;
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig};
use support::{corrections, Error};

const WIDTH: i32 = 6;
/// Blocks of river per fill and teleport (the chunks must be loaded for a fill).
const PIECE: i32 = 48;
/// A piece centred this near the bot is filled without a teleport.
const NEAR: i32 = 24;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(900);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19190".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Boater".into());
    let num = |i: usize, default: i32| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(default);
    let (a, b, c) = (num(2, 200), num(3, 60), num(4, 240));
    let profile = match std::env::var("BEDROCK_PROFILE").as_deref() {
        Ok("human") => MovementProfile::Human,
        _ => MovementProfile::Optimal,
    };
    let config = BotConfig { trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let (mut bot, _pad) = support::connect_config(&server, &name, config).await?;
    let start = bot.block_position().ok_or("no position")?;
    let [x0, y, z0] = start;
    // Water fills the two blocks under the walking surface; banks stay grass.
    let legs = [([x0 + 1, z0 - WIDTH / 2], [x0 + a, z0 + WIDTH / 2 - 1]), ([x0 + a - WIDTH + 1, z0 - WIDTH / 2], [x0 + a, z0 + b]),
        ([x0 + a - WIDTH + 1, z0 + b - WIDTH + 1], [x0 + a + c, z0 + b])];
    for &(min, max) in &legs {
        dig(&mut bot, min, max, y).await?;
    }
    if bot.block_position() != Some(start) {
        run(&mut bot, &format!("/tp @s {} {y} {}", x0, z0)).await?;
    }
    // SUMMON_BOAT=1 floats an empty boat beside the bot instead of giving one to place.
    let source = match std::env::var_os("SUMMON_BOAT") {
        Some(_) => format!("/summon boat {}.5 {y} {}.5", x0 + 2, z0),
        None => "/give @s oak_boat".to_owned(),
    };
    for cmd in ["/gamemode survival @s", "/kill @e[type=boat]", "/clear @s", source.as_str()] {
        run(&mut bot, cmd).await?;
    }
    bot.wait_ticks(40).await?;
    let target = [x0 + a + c + 1, y, z0 + b - WIDTH / 2];
    let (before, boat_before, t0) = (corrections(&bot), bot.boat_corrections(), std::time::Instant::now());
    let opts = BoatOpts { profile, keep_boat: std::env::var_os("KEEP_BOAT").is_some(), ..BoatOpts::default() };
    let result = bot.boat_to(target, opts).await;
    let feet = bot.movement().and_then(|m| m.position()).ok_or("no position")?;
    let off = (feet[0] - (target[0] as f32 + 0.5)).hypot(feet[2] - (target[2] as f32 + 0.5));
    let (height, horizontal) = bot.boat_corrections();
    let boats = bot.state().inventory.main.iter().filter(|s| !s.is_empty() && bot.state().items.name(s.network_id).is_some_and(|n| n.ends_with("boat"))).count();
    println!(
        "RESULT {profile:?} {result:?}: boat corrections {} height only, {} horizontal; player {}; {:.1} s; ended {off:.2} from {target:?} at {feet:?}; boats held {boats}",
        height - boat_before.0,
        horizontal - boat_before.1,
        corrections(&bot) - before,
        t0.elapsed().as_secs_f32()
    );
    Ok(())
}

async fn run(bot: &mut Bot, cmd: &str) -> Result<(), Error> {
    bot.client().command(cmd);
    bot.wait_ticks(4).await?;
    Ok(())
}

/// Turns the two blocks under the surface at `y` into water over the x/z box, a piece at a time with the bot
/// teleported beside each piece so its chunks are loaded.
async fn dig(bot: &mut Bot, min: [i32; 2], max: [i32; 2], y: i32) -> Result<(), Error> {
    let (mut x, mut z) = (min[0], min[1]);
    while x <= max[0] {
        let x1 = (x + PIECE - 1).min(max[0]);
        while z <= max[1] {
            let z1 = (z + PIECE - 1).min(max[1]);
            let (cx, cz) = ((x + x1) / 2, (z + z1) / 2);
            let here = bot.block_position().ok_or("no position")?;
            if (cx - here[0]).abs().max((cz - here[2]).abs()) > NEAR {
                run(bot, &format!("/tp @s {cx} {} {cz}", y + 3)).await?;
                bot.wait_ticks(30).await?;
            }
            run(bot, &format!("/fill {x} {} {z} {x1} {} {z1} water", y - 2, y - 1)).await?;
            z = z1 + 1;
        }
        (x, z) = (x1 + 1, min[1]);
    }
    Ok(())
}
