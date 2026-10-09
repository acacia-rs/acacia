//! Dimension travel drill on a strict server-authoritative BDS where the bot is operator (docs/DESIGN.md
//! "Dimension travel"): overworld → Nether → overworld → End → overworld by `execute in <dimension> run tp`, each
//! onto a glass floor a first pass builds, then the same round through a Nether portal and End portal blocks set
//! by command. After each change it prints the server corrections of the arrival and of a walk, and how far the
//! simulation stands from the server's own position (a `tp ~ ~ ~` echo).
//! `cargo run -p acacia-bot --example dimensions -- <server> <name|@account> [tp|portals|all] [passes]`
//! `BEDROCK_RECORD=<file>` records a movement trace for the `replay` example.
mod support;

use acacia_bot::movement::Controls;
use acacia_bot::Bot;
use support::{corrections, Error};

const NAMES: [&str; 3] = ["overworld", "nether", "the_end"];
/// The floor block under the arrival point, per dimension: high over a flat or normal overworld, inside the
/// Nether's rock (the room is carved) and over the End's main island.
const FLOORS: [[i32; 3]; 3] = [[40, 100, 40], [40, 70, 40], [40, 80, 40]];
/// overworld → Nether → overworld → End → overworld.
const TOUR: [usize; 4] = [1, 0, 2, 0];
const ARRIVAL_TIMEOUT_TICKS: u32 = 1200;
/// Where BDS puts a player entering the End: on the obsidian platform.
const END_PLATFORM: [f32; 3] = [100.0, 50.0, 0.0];

const PORTAL_COOLDOWN_TICKS: u32 = 320;

type Feet = [f32; 3];

fn run(bot: &Bot, dimension: usize, command: &str) {
    bot.client().command(&format!("/execute in {} run {command}", NAMES[dimension]));
}

fn dist(a: Feet, b: Feet) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f32>().sqrt()
}

fn feet(bot: &Bot) -> Result<Feet, Error> {
    Ok(bot.movement().and_then(|m| m.position()).ok_or("movement stopped")?)
}

fn on_floor(dimension: usize, dx: i32) -> Feet {
    let [x, y, z] = FLOORS[dimension];
    [(x + dx) as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5]
}

/// Teleports to `target` in the dimension the bot is in and waits for the teleport.
async fn teleport(bot: &mut Bot, target: Feet) -> Result<(), Error> {
    let teleports = bot.movement().map_or(0, |m| m.teleports);
    bot.client().command(&format!("/tp @s {} {} {} -90 0", target[0], target[1], target[2]));
    for _ in 0..200 {
        if bot.movement().is_some_and(|m| m.teleports > teleports) {
            return Ok(bot.wait_ticks(5).await?);
        }
        bot.wait_ticks(1).await?;
    }
    Err("no teleport".into())
}

/// Waits until the simulation runs in `dimension` after a change (which stops it until the bot has arrived).
/// Returns where it started.
async fn arrival(bot: &mut Bot, dimension: usize) -> Result<Feet, Error> {
    let (mut left, mut stopped) = (ARRIVAL_TIMEOUT_TICKS, false);
    loop {
        let started = bot.movement().is_some_and(|m| m.is_started());
        stopped |= !started;
        if stopped && started && bot.state().player.dimension == dimension as i32 {
            return feet(bot);
        }
        left = left.checked_sub(1).ok_or_else(|| format!("no arrival in {}", NAMES[dimension]))?;
        bot.wait_ticks(1).await?;
    }
}

/// Goes onto the dimension's floor by command.
async fn travel(bot: &mut Bot, dimension: usize) -> Result<Feet, Error> {
    let target = on_floor(dimension, 0);
    if bot.state().player.dimension == dimension as i32 {
        teleport(bot, target).await?;
        return feet(bot);
    }
    run(bot, dimension, &format!("tp @s {} {} {} -90 0", target[0], target[1], target[2]));
    arrival(bot, dimension).await
}

/// A glass floor with four blocks of air over it, long enough for the walk (+X).
async fn build_room(bot: &mut Bot, dimension: usize) -> Result<(), Error> {
    let [x, y, z] = FLOORS[dimension];
    run(bot, dimension, &format!("fill {} {} {} {} {} {} air", x - 3, y + 1, z - 3, x + 12, y + 4, z + 3));
    bot.wait_ticks(2).await?;
    run(bot, dimension, &format!("fill {} {y} {} {} {y} {} glass", x - 3, z - 3, x + 12, z + 3));
    if dimension == 2 {
        // The dragon knocks players about, and peaceful difficulty does not remove it.
        bot.client().command("/kill @e[type=ender_dragon]");
    }
    Ok(bot.wait_ticks(20).await?)
}

/// How far the simulation is from where the server has the player: `tp ~ ~ ~` answers with a `MovePlayer` to
/// the server's own position.
async fn server_offset(bot: &mut Bot) -> Result<f32, Error> {
    let (ours, teleports) = (feet(bot)?, bot.movement().map_or(0, |m| m.teleports));
    bot.client().command("/tp @s ~ ~ ~");
    for _ in 0..200 {
        if bot.movement().is_some_and(|m| m.teleports > teleports) {
            let p = &bot.state().player.position;
            let off = dist(ours, [p.x, p.y, p.z]);
            bot.wait_ticks(5).await?;
            return Ok(off);
        }
        bot.wait_ticks(1).await?;
    }
    Err("the server did not echo the position".into())
}

/// Walks +X; sneaking where the ground around is not ours (a portal the game built).
async fn walk(bot: &mut Bot, ticks: u32, sneak: bool) -> Result<(), Error> {
    for (ticks, forward) in [(ticks, 1.0), (15, 0.0)] {
        *bot.controls().ok_or("no physics")? = Controls { forward, sneak, yaw: -90.0, ..Controls::default() };
        bot.wait_ticks(ticks).await?;
    }
    Ok(())
}

/// The measured part of a leg, from the tick its change was triggered: prints the corrections of the arrival
/// and of a walk, and the offsets from the server. Returns the corrections.
async fn measure(bot: &mut Bot, name: &str, start: u32, to: usize, target: Option<Feet>, sneak: bool) -> Result<u32, Error> {
    let at = arrival(bot, to).await?;
    bot.wait_ticks(20).await?;
    let arriving = corrections(bot) - start;
    let stood = server_offset(bot).await?;
    let (before, stand) = (corrections(bot), feet(bot)?);
    walk(bot, if sneak { 10 } else { 30 }, sneak).await?;
    let walking = corrections(bot) - before;
    let walked = dist(stand, feet(bot)?);
    let ended = server_offset(bot).await?;
    let landed = target.map_or(format!("at {at:.2?}"), |t| format!("{:.4} from the target", dist(at, t)));
    println!(
        "{name:26} landed {landed}; arriving: corrections {arriving}, off the server by {stood:.4}; \
         walking {walked:.2} blocks: corrections {walking}, off by {ended:.4}"
    );
    Ok(arriving + walking)
}

/// One leg by command.
async fn leg(bot: &mut Bot, from: usize, to: usize) -> Result<u32, Error> {
    let name = format!("{} > {}", NAMES[from], NAMES[to]);
    bot.trace_mark(&name);
    let (start, target) = (corrections(bot), on_floor(to, 0));
    run(bot, to, &format!("tp @s {} {} {} -90 0", target[0], target[1], target[2]));
    measure(bot, &name, start, to, Some(target), false).await
}

/// One leg through a portal: stands in the portal block at `portal` until the game moves the bot to `to`.
async fn portal_leg(bot: &mut Bot, kind: &str, portal: Feet, to: usize, target: Option<Feet>) -> Result<u32, Error> {
    let name = format!("{kind} > {}", NAMES[to]);
    bot.trace_mark(&name);
    teleport(bot, portal).await?;
    measure(bot, &name, corrections(bot), to, target, true).await
}

/// Overworld → Nether → overworld through a Nether portal built on the overworld floor (a survival player
/// stands in it for 4 s), then overworld → End → overworld through End portal blocks.
async fn portals(bot: &mut Bot) -> Result<u32, Error> {
    let [x, y, z] = FLOORS[0];
    travel(bot, 0).await?;
    // The frame stands across the walk (+X), its two portal blocks on z-1 and z.
    run(bot, 0, &format!("fill {} {y} {} {} {} {} obsidian", x + 6, z - 2, x + 6, y + 4, z + 1));
    bot.wait_ticks(2).await?;
    run(bot, 0, &format!("fill {} {} {} {} {} {z} portal [\"portal_axis\"=\"z\"]", x + 6, y + 1, z - 1, x + 6, y + 3));
    // On a floor of its own: a player falling through a portal block is not taken (live, 1.26.52).
    run(bot, 0, &format!("setblock {} {} {} glass", x + 3, y - 1, z + 2));
    run(bot, 0, &format!("setblock {} {y} {} end_portal", x + 3, z + 2));
    bot.wait_ticks(20).await?;
    let mut total = portal_leg(bot, "nether portal", on_floor(0, 6), 1, None).await?;
    // Back through the portal the game made, after a while outside it: a portal just used does nothing.
    let made = bot.state().player.position.clone();
    teleport(bot, on_floor(1, 0)).await?;
    bot.wait_ticks(PORTAL_COOLDOWN_TICKS).await?;
    total += portal_leg(bot, "nether portal", [made.x, made.y, made.z], 0, None).await?;
    bot.wait_ticks(PORTAL_COOLDOWN_TICKS).await?;
    let end_portal = [(x + 3) as f32 + 0.5, y as f32 + 0.2, (z + 2) as f32 + 0.5];
    total += portal_leg(bot, "end portal", end_portal, 2, Some(END_PLATFORM)).await?;
    // The way back is a portal block beside the platform; the game returns the player to its spawn point.
    run(bot, 2, "setblock 103 48 0 glass");
    run(bot, 2, "setblock 103 49 0 end_portal");
    bot.wait_ticks(10).await?;
    total += portal_leg(bot, "end portal", [103.5, 49.2, 0.5], 0, None).await?;
    Ok(total)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Traveller".into());
    let mode = args.next().unwrap_or_else(|| "all".into());
    let passes: u32 = args.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let (mut bot, pad) = support::connect(&server, &name).await?;

    for dimension in [0, 1, 2, 0] {
        travel(&mut bot, dimension).await?;
        build_room(&mut bot, dimension).await?;
    }
    travel(&mut bot, 0).await?;
    let (mut total, start) = (0, corrections(&bot));
    for _ in 0..passes {
        let mut from = 0;
        for to in TOUR.into_iter().filter(|_| mode != "portals") {
            total += leg(&mut bot, from, to).await?;
            from = to;
        }
        if mode != "tp" {
            total += portals(&mut bot).await?;
        }
    }
    // The echo and set-up teleports land mid-tick and can be corrected themselves; those are not the travel's.
    println!("total corrections {total} (and {} around the other teleports)", corrections(&bot) - start - total);
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}
