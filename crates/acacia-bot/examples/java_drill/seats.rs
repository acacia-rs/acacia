//! Seat drills. Grim never sees a passenger's position and Paper's stored passenger `Pos` lags (it kept the pre-boarding
//! height for whole stages), so each checks the seat against vanilla's geometry. `camel_sit`: a saddled NoAI camel
//! summoned sitting; forward stands it up, and the seat must rise only, over the 52-tick stand-up, from the sitting
//! height to the standing one. `scaled_rider`: a player at scale 0.5 in a boat sits 0.3 lower than its 0.6
//! attachment.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["camel_sit", "scaled_rider"];

const SETTLE_TICKS: u32 = 60;
/// Feet within this of the expected seat.
const SAME: f64 = 1e-4;
/// A rider's feet above an adult camel: sat, `0.945 - 0.375 + 0.2 - 0.6`; standing, `2.375 - 0.375 - 0.6`.
const SAT: f64 = 0.17;
const STOOD: f64 = 1.4;
/// Above a boat for a half-size rider: `0.5625 / 3 - 0.6 * 0.5`.
const SMALL_IN_BOAT: f64 = 0.1875 - 0.3;

/// The feet above the driven vehicle's once the seat settles.
async fn seat_height(d: &mut Drill) -> Result<f64, String> {
    d.ticks(SETTLE_TICKS).await?;
    let c = d.bot.java().map(|j| j.client()).ok_or("no java client")?;
    let vehicle = c.movement.riding.as_ref().and_then(|r| r.driven.as_ref()).ok_or("not driving")?;
    Ok(c.movement.player.pos[1] - vehicle.pose().0[1])
}

/// Whether the seat only rose over `n` ticks, and did rise.
async fn track_rise(d: &mut Drill, n: u32) -> Result<bool, String> {
    let (mut up, mut down, mut last) = (0, 0, d.feet()[1]);
    for _ in 0..n {
        d.ticks(1).await?;
        let f = d.feet()[1];
        up += u32::from(f > last);
        down += u32::from(f < last);
        last = f;
    }
    Ok(up > 0 && down == 0)
}

async fn board(d: &mut Drill, kind: &str) -> Result<(), String> {
    let types = &acacia_java_proto::entity_types::ENTITY_TYPES;
    let named = |k: i32| usize::try_from(k).ok().and_then(|k| types.get(k)).is_some_and(|n| n.ends_with(kind));
    let world = d.bot.java().map(|j| &j.client().world);
    let id = world.and_then(|w| w.entities.iter().find(|(_, e)| named(e.kind)).map(|(&id, _)| id as u64)).ok_or(format!("no {kind} tracked"))?;
    d.bot.mount(id).await.map_err(|e| format!("boarding: {e}"))?;
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, 0.0);
    }
    Ok(())
}

async fn get_off(d: &mut Drill) -> Result<bool, String> {
    d.bot.dismount().await.map_err(|e| format!("dismount: {e}"))?;
    d.ticks(40).await?;
    Ok(d.bot.vehicle().is_none())
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let y = FLOOR + 1;
    let near = |a: f64, b: f64| (a - b).abs() < SAME;
    match name {
        "camel_sit" => {
            let saddle = "equipment:{saddle:{id:\"minecraft:saddle\",count:1}}";
            let camel = format!("summon camel {}.5 {y} 0.5 {{NoAI:1b,LastPoseTick:-1L,Rotation:[-90f,0f],{saddle}}}", x + 2);
            let base = start(d, x, &[&camel]).await?;
            board(d, ":camel").await?;
            let low = seat_height(d).await?;
            d.walk(3, 1.0, false, false).await?;
            let rise = track_rise(d, 55).await?;
            let high = seat_height(d).await?;
            let off = get_off(d).await?;
            let ok = near(low, SAT) && near(high, STOOD) && rise && off;
            Ok((base, check(ok, || format!("seat {low:.5} → {high:.5}, only rising {rise}, off {off}"))))
        }
        "scaled_rider" => {
            let boat = format!("summon oak_boat {}.5 {y} 0.5 {{Rotation:[-90f,0f]}}", x + 2);
            let base = start(d, x, &["attribute @s minecraft:scale base set 0.5", &boat]).await?;
            board(d, ":oak_boat").await?;
            let seat = seat_height(d).await?;
            let off = get_off(d).await?;
            d.bot.command("attribute @s minecraft:scale base reset");
            d.ticks(10).await?;
            Ok((base, check(near(seat, SMALL_IN_BOAT) && off, || format!("feet {seat:.5} above the boat, off {off}"))))
        }
        n => Err(format!("no drill {n}")),
    }
}
