//! Vehicle drills on a strict server-authoritative BDS where the bot is operator, for recorded traces
//! (docs/physics-gaps.md): a boat paddled on water, ice, packed ice, blue ice and stone, and a tamed horse
//! walking, turning and taking charged jumps over 1- and 2-block steps. Each phase rebuilds a floating
//! arena, summons the vehicle, mounts it, holds its key segments and dismounts; it prints the vehicle and
//! player corrections of the phase.
//! `cargo run -p acacia-bot --example vehicle_drill -- <server> <name> [phases,comma,separated]`
//! Run with `CHUNK_RADIUS=6` so the whole arena is loaded; `BEDROCK_RECORD=<file>` records the trace.
mod support;

use acacia_bot::client::Client;
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig, BotEvent};
use support::{corrections, Error};

/// The second player who takes a boat's front seat, so the drilled bot rides behind.
const FRONT_RIDER: &str = "FrontRider";

/// Each phase builds a fresh arena (rebuilt floors did not always reach the client) this far east of the last;
/// its floor is UP above home and spans x ±HALF and z from -BACK to +AHEAD.
const SPACING: i32 = 48;
const UP: i32 = 24;
const HALF: i32 = 14;
const BACK: i32 = 6;
const AHEAD: i32 = 64;
/// Ticks a boarded boat sits before paddling (BDS's paddle clock starts no fresh stroke sooner; riding/boat.rs).
const BOAT_SETTLE: u32 = 12;

#[derive(Clone, Copy)]
enum Floor {
    Water,
    Lava,
    Block(&'static str),
}

#[derive(Clone, Copy)]
enum Vehicle {
    Boat,
    /// A boat with a pig in the front seat: the bot rides behind as a passenger.
    BoatBack,
    Horse,
    Camel,
    /// Steered with a warped fungus on a stick; the server moves it.
    Strider,
}

/// (forward, strafe, jump, rider yaw, ticks)
type Segment = (f32, f32, bool, f32, u32);

struct Phase {
    name: &'static str,
    floor: Floor,
    vehicle: Vehicle,
    /// A wall across the path this many blocks ahead, this high (horse jumps).
    step: Option<(i32, i32)>,
    segments: &'static [Segment],
}

const BOAT_STRAIGHT: &[Segment] = &[(1.0, 0.0, false, 0.0, 60), (0.0, 0.0, false, 0.0, 30)];
const BOAT_TURN: &[Segment] =
    &[(1.0, 1.0, false, 0.0, 30), (1.0, -1.0, false, 0.0, 30), (0.0, 1.0, false, 0.0, 20), (0.0, -1.0, false, 0.0, 20), (-1.0, 0.0, false, 0.0, 20), (0.0, 0.0, false, 0.0, 20)];
const BOAT_SLIDE: &[Segment] = &[(1.0, 0.0, false, 0.0, 25), (1.0, 1.0, false, 0.0, 10), (0.0, 0.0, false, 0.0, 30)];
const HORSE_WALK: &[Segment] =
    &[(1.0, 0.0, false, 0.0, 40), (1.0, 0.0, false, 90.0, 30), (0.0, 1.0, false, 90.0, 15), (-1.0, 0.0, false, 90.0, 15), (0.0, 0.0, false, 90.0, 10)];
/// Walk up to the wall, charge the jump while walking, release before it, land and walk on.
const HORSE_JUMP: &[Segment] = &[(1.0, 0.0, false, 0.0, 4), (1.0, 0.0, true, 0.0, 10), (1.0, 0.0, false, 0.0, 30), (0.0, 0.0, false, 0.0, 10)];
const HORSE_STAND_JUMP: &[Segment] = &[(0.0, 0.0, true, 0.0, 6), (0.0, 0.0, false, 0.0, 20), (0.0, 0.0, true, 0.0, 15), (0.0, 0.0, false, 0.0, 30)];

const PHASES: &[Phase] = &[
    Phase { name: "boat_water", floor: Floor::Water, vehicle: Vehicle::Boat, step: None, segments: BOAT_STRAIGHT },
    Phase { name: "boat_water_turn", floor: Floor::Water, vehicle: Vehicle::Boat, step: None, segments: BOAT_TURN },
    Phase { name: "boat_ice", floor: Floor::Block("ice"), vehicle: Vehicle::Boat, step: None, segments: BOAT_SLIDE },
    Phase { name: "boat_packed_ice", floor: Floor::Block("packed_ice"), vehicle: Vehicle::Boat, step: None, segments: BOAT_SLIDE },
    Phase { name: "boat_blue_ice", floor: Floor::Block("blue_ice"), vehicle: Vehicle::Boat, step: None, segments: BOAT_SLIDE },
    Phase { name: "boat_stone", floor: Floor::Block("stone"), vehicle: Vehicle::Boat, step: None, segments: BOAT_SLIDE },
    Phase { name: "horse_walk", floor: Floor::Block("stone"), vehicle: Vehicle::Horse, step: None, segments: HORSE_WALK },
    Phase { name: "horse_stand_jump", floor: Floor::Block("stone"), vehicle: Vehicle::Horse, step: None, segments: HORSE_STAND_JUMP },
    Phase { name: "horse_jump1", floor: Floor::Block("stone"), vehicle: Vehicle::Horse, step: Some((6, 1)), segments: HORSE_JUMP },
    Phase { name: "horse_jump2", floor: Floor::Block("stone"), vehicle: Vehicle::Horse, step: Some((6, 2)), segments: HORSE_JUMP },
    Phase { name: "horse_ice", floor: Floor::Block("ice"), vehicle: Vehicle::Horse, step: None, segments: HORSE_WALK },
    Phase { name: "camel_walk", floor: Floor::Block("stone"), vehicle: Vehicle::Camel, step: None, segments: HORSE_WALK },
    Phase { name: "camel_dash", floor: Floor::Block("stone"), vehicle: Vehicle::Camel, step: None, segments: HORSE_JUMP },
    Phase { name: "boat_back", floor: Floor::Water, vehicle: Vehicle::BoatBack, step: None, segments: BOAT_STRAIGHT },
    Phase { name: "strider", floor: Floor::Lava, vehicle: Vehicle::Strider, step: None, segments: HORSE_WALK },
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run_phases()).await
}

async fn run_phases() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(1200);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19190".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Driver".into());
    let only: Option<Vec<&str>> = args.get(2).map(|s| s.split(',').collect());
    let record = std::env::var_os("BEDROCK_RECORD").map(Into::into);
    let config = BotConfig { record, trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let (mut bot, _pad) = support::connect_config(&server, &name, config).await?;
    // Creative: the bot falls to the ground while each new arena's chunks load, unharmed.
    for cmd in ["/difficulty peaceful", "/gamemode creative @s", "/clear @s"] {
        run(&mut bot, cmd).await?;
    }
    let home = bot.block_position().ok_or("no position")?;
    let mut totals = (0, 0);
    for (i, p) in PHASES.iter().enumerate().filter(|(_, p)| only.as_ref().is_none_or(|o| o.contains(&p.name))) {
        let centre = [home[0] + SPACING * i as i32, home[1] + UP, home[2]];
        run(&mut bot, &format!("/tp @s {}.5 {} {}.5", centre[0] + 2, centre[1] + 1, centre[2])).await?;
        bot.wait_ticks(60).await?;
        let (vehicle, player) = drive(&mut bot, &server, p, centre).await?;
        println!("{:>18}: {vehicle} vehicle corrections, {player} player corrections", p.name);
        totals = (totals.0 + vehicle, totals.1 + player);
    }
    println!("TOTAL {} vehicle corrections, {} player corrections", totals.0, totals.1);
    Ok(())
}

/// One phase: arena, vehicle, mount, segments, dismount. Returns (vehicle, player) corrections.
async fn drive(bot: &mut Bot, server: &str, p: &Phase, [x, y, z]: [i32; 3]) -> Result<(u32, u32), Error> {
    for cmd in arena(p, [x, y, z]) {
        run(bot, &cmd).await?;
    }
    // The bot stands on a stone pier east of the vehicle.
    run(bot, &format!("/tp @s {}.5 {} {}.5 90 0", x + 2, y + 1, z)).await?;
    let (kind, summon) = match p.vehicle {
        // Yaw 90 points the bow south (+z): the bow is 90° left of a boat's yaw.
        Vehicle::Boat | Vehicle::BoatBack => ("minecraft:boat", format!("/summon boat {x}.5 {} {z}.5 90 0", y + 1)),
        Vehicle::Horse => ("minecraft:horse", format!("/summon horse {x}.5 {} {z}.5 0 0 minecraft:on_tame", y + 1)),
        Vehicle::Camel => ("minecraft:camel", format!("/summon camel {x}.5 {} {z}.5 0 0", y + 1)),
        Vehicle::Strider => ("minecraft:strider", format!("/summon strider {x}.5 {} {z}.5 0 0 minecraft:on_saddled", y + 1)),
    };
    run(bot, &summon).await?;
    // BDS seats no player behind a mob in a boat (vehicle6/7: pig by /ride, then click or /ride), so a second player
    // drives.
    let front = match p.vehicle {
        Vehicle::Boat => None,
        Vehicle::BoatBack => {
            let rider = front_rider(server).await?;
            bot.wait_ticks(40).await?;
            run(bot, &format!("/tp {FRONT_RIDER} {}.5 {} {z}.5", x + 2, y + 1)).await?;
            bot.wait_ticks(60).await?;
            Some(rider)
        }
        _ => {
            run(bot, &format!("/replaceitem entity @e[type={}] slot.saddle 0 saddle", kind.trim_start_matches("minecraft:"))).await?;
            None
        }
    };
    bot.wait_ticks(20).await?;
    println!("{:>18}: floor under the vehicle as the client sees it: {}", p.name, client_block(bot, [x, y, z]));
    let me = bot.state().player.position.clone();
    let id = bot.state().entities.nearest(&me, |e| e.kind == kind).map(|e| e.runtime_id).ok_or(format!("no {kind} tracked"))?;
    bot.bedrock_mut().expect("Bedrock bot").trace_mark(p.name);
    // A camel sometimes ignores the first click (vehicle4); try again.
    let mut tries = 0;
    while let Err(e) = bot.mount(id).await {
        tries += 1;
        if tries == 3 {
            return Err(e.into());
        }
        bot.wait_ticks(20).await?;
    }
    // Held while clicking, the stick is used instead of the strider mounted (vehicle8).
    if let Vehicle::Strider = p.vehicle {
        run(bot, "/give @s warped_fungus_on_a_stick").await?;
        bot.select_hotbar(0)?;
    }
    bot.wait_ticks(if let Vehicle::Boat = p.vehicle { BOAT_SETTLE } else { 5 }).await?;
    let (vehicle_before, player_before) = (vehicle_corrections(bot), corrections(bot));
    for &(forward, strafe, jump, yaw, ticks) in p.segments {
        if let Some(c) = bot.controls() {
            (c.forward, c.strafe, c.jump, c.yaw, c.pitch) = (forward, strafe, jump, yaw, 0.0);
        }
        let start = vehicle_position(bot, id);
        bot.wait_ticks(ticks).await?;
        println!("{:>18}: segment f{forward} s{strafe} j{jump} y{yaw}: vehicle {start:?} -> {:?}", p.name, vehicle_position(bot, id));
    }
    if let Some(c) = bot.controls() {
        c.stop();
    }
    let vehicle = vehicle_corrections(bot) - vehicle_before;
    bot.dismount().await?;
    // Step clear of the vehicle before it is removed (an empty boat drifting into the player pushes it).
    if let Some(c) = bot.controls() {
        (c.forward, c.yaw) = (1.0, -90.0);
    }
    bot.wait_ticks(3).await?;
    if let Some(c) = bot.controls() {
        c.stop();
    }
    bot.wait_ticks(10).await?;
    let player = corrections(bot) - player_before;
    if let Some(rider) = front {
        rider.abort();
    }
    run(bot, &format!("/kill @e[type={}]", kind.trim_start_matches("minecraft:"))).await?;
    for cmd in ["/kill @e[type=item]", "/clear @s"] {
        run(bot, cmd).await?;
    }
    Ok((vehicle, player))
}

/// Connects [`FRONT_RIDER`], who gets into the first boat within 4 blocks once teleported to one and stays seated
/// until the task is dropped.
async fn front_rider(server: &str) -> Result<tokio::task::JoinHandle<()>, Error> {
    let config = BotConfig { trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let mut b = Bot::connect(Client::builder(server).offline(FRONT_RIDER), config).await?;
    Ok(tokio::task::spawn_local(async move {
        loop {
            if b.wait_ticks(5).await.is_err() {
                return;
            }
            let me = b.state().player.position.clone();
            let near = |e: &acacia_bot::state::Entity| {
                e.kind == "minecraft:boat" && (e.position.x - me.x).hypot(e.position.z - me.z) < 4.0
            };
            if let Some(id) = b.state().entities.nearest(&me, near).map(|e| e.runtime_id)
                && b.mount(id).await.is_ok()
            {
                break;
            }
        }
        while let Some(e) = b.next().await {
            if let BotEvent::Disconnected(_) = e {
                break;
            }
        }
    }))
}

/// Clears the arena and lays the phase's floor (a 2-deep pool in a stone basin for water), the pier and
/// the step.
fn arena(p: &Phase, [x, y, z]: [i32; 3]) -> Vec<String> {
    let (x0, x1, z0, z1) = (x - HALF, x + HALF, z - BACK, z + AHEAD);
    let mut cmds = vec![format!("/fill {x0} {} {z0} {x1} {} {z1} air", y + 1, y + 6), format!("/fill {x0} {} {z0} {x1} {y} {z1} stone", y - 3)];
    match p.floor {
        Floor::Water => cmds.push(format!("/fill {} {} {} {} {y} {} water", x0 + 1, y - 2, z0 + 1, x1 - 1, z1 - 1)),
        Floor::Lava => cmds.push(format!("/fill {} {} {} {} {y} {} lava", x0 + 1, y - 2, z0 + 1, x1 - 1, z1 - 1)),
        Floor::Block(block) => cmds.push(format!("/fill {x0} {y} {z0} {x1} {y} {z1} {block}")),
    }
    cmds.push(format!("/setblock {} {y} {z} stone", x + 2));
    if let Some((ahead, high)) = p.step {
        cmds.push(format!("/fill {x0} {} {} {x1} {} {} stone", y + 1, z + ahead, y + high, z + AHEAD));
    }
    cmds
}

fn client_block(bot: &Bot, [x, y, z]: [i32; 3]) -> String {
    use acacia_world::BlockAccess;
    let Some(w) = bot.bedrock().and_then(|b| b.world()) else { return "?".into() };
    let (Some(view), Some(r)) = (w.view(), w.registry()) else { return "?".into() };
    r.get(view.block(x, y, z)).map_or("?".into(), |s| s.name.to_string())
}

/// The vehicle's position as the server last sent it.
fn vehicle_position(bot: &Bot, id: u64) -> Option<[f32; 3]> {
    let me = bot.state().player.position.clone();
    bot.state().entities.nearest(&me, |e| e.runtime_id == id).map(|e| [e.position.x, e.position.y, e.position.z])
}

fn vehicle_corrections(bot: &Bot) -> u32 {
    bot.bedrock().and_then(|b| b.movement()).map_or(0, |m| m.vehicle_corrections)
}

async fn run(bot: &mut Bot, cmd: &str) -> Result<(), Error> {
    bot.command(cmd);
    bot.wait_ticks(4).await?;
    Ok(())
}
