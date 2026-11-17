//! Seats past the first, checked against vanilla's seat geometry (Grim never sees a passenger). `boat_pair`: the bot
//! drives a boat a villager then boards, so the driver sits 0.2 forward, and paddles. `boat_back_seat` and
//! `camel_back_seat`: a second player (`Peer`) drives and the bot rides behind (a boat's 0.6 back, a camel's 0.7 back
//! at its back anchor), its yaw held within 105° of a boat's; when the peer gets off, the bot takes the controls.

use acacia_java_physics::seat_feet;

use crate::crowd::both;
use crate::drills::{check, start, Outcome};
use crate::seats::{board, get_off, SAME};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["boat_pair", "boat_back_seat", "camel_back_seat"];

const PEER: &str = "Peer";
/// `Boat.rideHeight`: a third of the boat's 0.5625.
const BOAT_SEAT: f64 = 0.1875;
/// A camel's standing anchor, `2.375 - 0.375`.
const CAMEL_SEAT: f64 = 2.0;
const ATTACHMENT: f64 = 0.6;

/// The bot's feet against `seat` (unturned) on the vehicle it rides at `pos`, `yaw`: the largest axis error.
fn seat_error(d: &Drill, pos: [f64; 3], yaw: f32, seat: [f64; 3]) -> f64 {
    let want = seat_feet(pos, seat, yaw, ATTACHMENT);
    let feet = d.feet();
    (0..3).map(|a| (feet[a] - want[a]).abs()).fold(0.0, f64::max)
}

/// The vehicle the peer drives, as the peer simulates it.
fn peer_vehicle(peer: &Drill) -> Option<([f64; 3], f32)> {
    let j = peer.bot.java()?;
    let (pos, yaw, _) = j.client().movement.riding.as_ref()?.driven.as_ref()?.pose();
    Some((pos, yaw))
}

/// How far the bot's yaw is turned from its vehicle's, as the bot's client shows that vehicle.
fn yaw_off_vehicle(d: &Drill) -> Option<f32> {
    let c = d.bot.java()?.client();
    let vehicle = c.world.entities.get(&c.movement.riding.as_ref()?.vehicle)?;
    Some((c.movement.player.yaw - vehicle.y_rot).rem_euclid(360.0).min((vehicle.y_rot - c.movement.player.yaw).rem_euclid(360.0)))
}

fn summon(kind: &str, x: i32, saddled: bool) -> String {
    let saddle = if saddled { ",equipment:{saddle:{id:\"minecraft:saddle\",count:1}}" } else { "" };
    format!("summon {kind} {}.5 {} 0.5 {{NoAI:1b,Rotation:[-90f,0f]{saddle}}}", x + 2, FLOOR + 1)
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    match name {
        "boat_pair" => {
            let villager = format!("summon villager {}.5 {} 2.5 {{NoAI:1b}}", x + 2, FLOOR + 1);
            let base = start(d, x, &[&summon("oak_boat", x, false), &villager]).await?;
            board(d, ":oak_boat").await?;
            d.bot.command("ride @e[type=villager,limit=1,sort=nearest] mount @e[type=oak_boat,limit=1,sort=nearest]");
            d.ticks(40).await?;
            let boat = |d: &Drill| d.bot.java_boat().map(|b| (b.pos, b.yaw)).ok_or("not driving");
            let (pos, yaw) = boat(d)?;
            let before = seat_error(d, pos, yaw, [0.0, BOAT_SEAT, 0.2]);
            d.walk(30, 1.0, false, false).await?;
            d.ticks(20).await?;
            let (pos, yaw) = boat(d)?;
            let after = seat_error(d, pos, yaw, [0.0, BOAT_SEAT, 0.2]);
            let off = get_off(d).await?;
            let ok = before < SAME && after < SAME && off;
            Ok((base, check(ok, || format!("driver seat off by {before:.5} and {after:.5} after paddling, off {off}"))))
        }
        "boat_back_seat" | "camel_back_seat" => {
            let boat = name == "boat_back_seat";
            let (kind, seat) = if boat { ("oak_boat", [0.0, BOAT_SEAT, -0.6]) } else { ("camel", [0.0, CAMEL_SEAT, -0.7]) };
            let base = start(d, x, &[&summon(kind, x, !boat)]).await?;
            let mut peer = Drill::connect(&d.server.clone(), PEER).await?;
            for c in [format!("gamemode survival {PEER}"), format!("tp {PEER} {x} {} 1 -90 0", FLOOR + 1)] {
                d.bot.command(&c);
                both(d, &mut peer, 3, 0.0).await?;
            }
            both(d, &mut peer, 30, 0.0).await?;
            let peer_base = peer.setbacks();
            board(&mut peer, &format!(":{kind}")).await?;
            both(d, &mut peer, 10, 0.0).await?;
            board(d, &format!(":{kind}")).await?;
            both(d, &mut peer, 40, 0.0).await?;
            let (pos, yaw) = peer_vehicle(&peer).ok_or("peer not driving")?;
            let sat = seat_error(d, pos, yaw, seat);
            both(d, &mut peer, 30, 1.0).await?;
            if let Some(c) = d.bot.controls() {
                c.yaw = 90.0;
            }
            both(d, &mut peer, 30, 0.0).await?;
            let (pos, yaw) = peer_vehicle(&peer).ok_or("peer not driving")?;
            let rode = seat_error(d, pos, yaw, seat);
            let turned = yaw_off_vehicle(d).unwrap_or(f32::MAX);
            if let Some(c) = d.bot.controls() {
                c.yaw = -90.0;
            }
            peer.bot.dismount().await.map_err(|e| format!("peer dismount: {e}"))?;
            both(d, &mut peer, 30, 0.0).await?;
            let took_over = d.bot.java().and_then(|j| j.client().movement.riding.as_ref().map(|r| r.driven.is_some())) == Some(true);
            d.walk(20, 1.0, false, false).await?;
            let off = get_off(d).await?;
            let peer_setbacks = peer.setbacks() - peer_base;
            peer.bot.disconnect().await;
            let clamped = !boat || turned <= 105.0 + 1e-3;
            let ok = sat < SAME && rode < SAME && clamped && took_over && off && peer_setbacks == 0;
            Ok((base, check(ok, || format!("back seat off by {sat:.5}, {rode:.5} after a ride, yaw {turned:.1}° off the vehicle, took over {took_over}, off {off}, peer setbacks {peer_setbacks}"))))
        }
        n => Err(format!("no drill {n}")),
    }
}
