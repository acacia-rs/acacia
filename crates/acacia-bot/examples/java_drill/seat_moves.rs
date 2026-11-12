//! Seats that move under a still rider, checked against vanilla's seat geometry. `horse_rear`: the bot charges a
//! jump on a saddled horse, which rears for 20 ticks (`standIfPossible`), and the seat swings 0.7 back
//! (`AbstractHorse.getPassengerAttachmentPoint` at `standAnimO` 1), then settles. `camel_sit_down`: the server sits a
//! ridden camel down (`/data merge` of a negative `LastPoseTick`, which `readAdditionalSaveData` turns into
//! `Pose.SITTING`), and the seat must only sink, over the 40-tick sit-down, to the sitting height. `strider_walk`: the
//! bot steers a saddled strider over grass with a warped fungus on a stick; the seat must follow the walk's bob, which
//! stays within `Strider.getPassengerAttachmentPoint`'s 0.06 either way. `horse_step`: a horse summoned with step height 2
//! climbs a two-block ledge, as the synced attribute lets it.

use crate::drills::{check, start, Outcome};
use crate::seats::{board, get_off, seat_height, SAME, SAT};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["horse_rear", "camel_sit_down", "strider_walk", "horse_step"];

const SADDLE: &str = "equipment:{saddle:{id:\"minecraft:saddle\",count:1}}";
const STOOD: f64 = 1.4;

/// The seat off the driven mount's position: facing east, a rear moves it west.
fn seat_off(d: &Drill) -> Result<[f64; 3], String> {
    let mount = d.bot.java_mount().ok_or("not driving")?;
    let feet = d.feet();
    Ok([0, 1, 2].map(|i| feet[i] - mount.body.pos[i]))
}

fn seat_x(d: &Drill) -> Result<f64, String> {
    Ok(seat_off(d)?[0])
}

/// Whether the seat only sank over `n` ticks, and did sink.
async fn track_sink(d: &mut Drill, n: u32) -> Result<bool, String> {
    let (mut up, mut down, mut last) = (0, 0, d.feet()[1]);
    for _ in 0..n {
        d.ticks(1).await?;
        let f = d.feet()[1];
        up += u32::from(f > last);
        down += u32::from(f < last);
        last = f;
    }
    Ok(down > 0 && up == 0)
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let y = FLOOR + 1;
    match name {
        "horse_rear" => {
            let horse = format!("summon horse {}.5 {y} 0.5 {{NoAI:1b,Tame:1b,Rotation:[-90f,0f],{SADDLE}}}", x + 2);
            let base = start(d, x, &[&horse]).await?;
            board(d, ":horse").await?;
            d.ticks(20).await?;
            let still = seat_x(d)?;
            d.walk(10, 0.0, true, false).await?;
            let mut back = 0.0_f64;
            for _ in 0..40 {
                d.ticks(1).await?;
                back = back.min(seat_x(d)?);
            }
            d.ticks(20).await?;
            let settled = seat_x(d)?;
            let off = get_off(d).await?;
            let ok = still.abs() < SAME && (back + 0.7).abs() < 0.01 && settled.abs() < SAME && off;
            Ok((base, check(ok, || format!("seat x {still:.5}, rearing {back:.5}, settled {settled:.5}, off {off}"))))
        }
        "camel_sit_down" => {
            let camel = format!("summon camel {}.5 {y} 0.5 {{NoAI:1b,Rotation:[-90f,0f],{SADDLE}}}", x + 2);
            let base = start(d, x, &[&camel]).await?;
            board(d, ":camel").await?;
            let stood = seat_height(d).await?;
            let now = d.bot.java().map_or(0, |j| j.client().world.game_time);
            d.bot.command(&format!("data merge entity @e[type=camel,limit=1,sort=nearest] {{LastPoseTick:-{now}L}}"));
            d.ticks(2).await?;
            let sank = track_sink(d, 45).await?;
            let sat = seat_height(d).await?;
            let synced = d.bot.java().and_then(|j| {
                let c = j.client();
                c.world.entities.get(&c.movement.riding.as_ref()?.vehicle).map(|e| (e.pose, e.mount.last_pose_change))
            });
            d.walk(3, 1.0, false, false).await?;
            d.ticks(60).await?;
            let off = get_off(d).await?;
            let ok = (stood - STOOD).abs() < SAME && sank && (sat - SAT).abs() < SAME && off;
            Ok((base, check(ok, || format!("seat {stood:.5} → {sat:.5}, only sinking {sank}, pose and pose tick {synced:?}, off {off}"))))
        }
        "strider_walk" => {
            let strider = format!("summon strider {}.5 {y} 0.5 {{NoAI:1b,Rotation:[-90f,0f],{SADDLE}}}", x + 2);
            let stick = "item replace entity @s weapon.mainhand with warped_fungus_on_a_stick";
            let base = start(d, x, &[&strider, stick]).await?;
            board(d, ":strider").await?;
            d.ticks(20).await?;
            let from = d.feet()[0];
            let (mut low, mut high, mut seats) = (f64::MAX, f64::MIN, Vec::new());
            for _ in 0..60 {
                d.walk(1, 1.0, false, false).await?;
                let bob = d.bot.java_mount().ok_or("not driving")?.anim.strider_bob(1.0);
                (low, high) = (low.min(bob), high.max(bob));
                seats.push(seat_off(d)?[1] - bob);
            }
            let went = d.feet()[0] - from;
            let off = get_off(d).await?;
            let drift = seats.iter().map(|s| (s - seats[0]).abs()).fold(0.0, f64::max);
            let ok = went > 2.0 && high - low > 0.01 && low >= -0.06 && high <= 0.06 && drift < SAME && off;
            Ok((base, check(ok, || format!("went {went:.3}, bob {low:.5}..{high:.5}, seat drift {drift:.5}, off {off}"))))
        }
        "horse_step" => {
            let attrs = "attributes:[{id:\"minecraft:step_height\",base:2.0}]";
            let horse = format!("summon horse {}.5 {y} 0.5 {{NoAI:1b,Tame:1b,Rotation:[-90f,0f],{attrs},{SADDLE}}}", x + 2);
            let ledge = format!("fill {} {y} -4 {} {} 4 stone", x + 6, x + 30, y + 1);
            let base = start(d, x, &[&ledge, &horse]).await?;
            board(d, ":horse").await?;
            d.ticks(20).await?;
            d.walk(60, 1.0, false, false).await?;
            let at = d.bot.java_mount().ok_or("not driving")?.body.pos;
            let off = get_off(d).await?;
            let ok = at[1] >= f64::from(y + 2) && at[0] > f64::from(x + 7) && off;
            Ok((base, check(ok, || format!("horse at {:.2} {:.2}, off {off}", at[0] - f64::from(x), at[1]))))
        }
        n => Err(format!("no drill {n}")),
    }
}
