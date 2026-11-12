//! Lid drills: a lid opens under the bot, or under what it drives, which must ride it up. Grim tolerates a missed push
//! near shulkers, so each checks the rise itself. `shulker_push`: a NoAI shulker on the floor peeks fully (its lid
//! rises a block over 20 ticks), then closes. `shulker_box_push`: the bot opens the shulker box it stands on (the lid
//! rises half a block and holds it up), then closes it. `boat_shulker_push` and `horse_lid_push` do the same under a
//! driven boat and horse: vanilla moves the entity in the lid's sweep, the vehicle, and the rider follows its seat (a
//! boat is solid, so a shulker box under it can't open; a horse isn't).

use crate::drills::{check, start, Outcome};
use crate::seats::{board, get_off};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["shulker_push", "shulker_box_push", "boat_shulker_push", "horse_lid_push"];

const SHULKER: &str = "@e[type=shulker,limit=1,sort=nearest]";

fn player_y(d: &Drill) -> f64 {
    d.feet()[1]
}

fn vehicle_y(d: &Drill) -> f64 {
    let driven = d.bot.java().and_then(|j| j.client().movement.riding.as_ref()?.driven.as_ref().map(|v| v.pose().0[1]));
    driven.unwrap_or(f64::NAN)
}

/// The highest and the last height over `n` ticks standing still.
async fn watch(d: &mut Drill, n: u32, height: fn(&Drill) -> f64) -> Result<(f64, f64), String> {
    let mut top = f64::MIN;
    for _ in 0..n {
        d.ticks(1).await?;
        top = top.max(height(d));
    }
    Ok((top, height(d)))
}

/// Peeks the nearest shulker fully, then shuts it: the highest, open and shut heights.
async fn peek(d: &mut Drill, height: fn(&Drill) -> f64) -> Result<(f64, f64, f64), String> {
    d.bot.command(&format!("data merge entity {SHULKER} {{Peek:100b}}"));
    let (high, open) = watch(d, 40, height).await?;
    d.bot.command(&format!("data merge entity {SHULKER} {{Peek:0b}}"));
    let (_, shut) = watch(d, 40, height).await?;
    d.bot.command("difficulty peaceful");
    Ok((high, open, shut))
}

/// Opens the shulker box at `pos`, then closes it: the highest, open and shut heights.
async fn open_box(d: &mut Drill, pos: [i32; 3], height: fn(&Drill) -> f64) -> Result<(f64, f64, f64), String> {
    d.bot.open_container_at(pos).await.map_err(|e| format!("opening: {e}"))?;
    let (high, open) = watch(d, 30, height).await?;
    d.bot.close_container().await.map_err(|e| format!("closing: {e}"))?;
    let (_, shut) = watch(d, 30, height).await?;
    Ok((high, open, shut))
}

fn verdict((high, open, shut): (f64, f64, f64), top: f64, rise: f64) -> Result<(), String> {
    let ok = high > top + rise - 0.05 && (open - (top + rise)).abs() < 1e-3 && (shut - top).abs() < 1e-3;
    check(ok, || format!("rose to {:.3}, rested at {:.3} open and {:.3} shut", high - top, open - top, shut - top))
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let (lid, top) = (FLOOR + 1, f64::from(FLOOR + 2));
    let stand = format!("tp @s {}.5 {top} 0.5 -90 0", x + 2);
    let shulker = format!("summon shulker {} {lid} 0 {{NoAI:1b,Peek:0b,AttachFace:0b}}", x + 2);
    let shulker_box = format!("setblock {} {lid} 0 shulker_box[facing=up]", x + 2);
    match name {
        "shulker_push" => {
            let base = start(d, x, &["difficulty easy", &shulker, &stand]).await?;
            let heights = peek(d, player_y).await?;
            d.walk(20, 1.0, false, false).await?;
            Ok((base, verdict(heights, top, 1.0)))
        }
        "shulker_box_push" => {
            let base = start(d, x, &[&shulker_box, &stand]).await?;
            let heights = open_box(d, [x + 2, lid, 0], player_y).await?;
            d.walk(20, 1.0, false, false).await?;
            Ok((base, verdict(heights, top, 0.5)))
        }
        "boat_shulker_push" => {
            let boat = format!("summon oak_boat {}.5 {top} 0.5 {{Rotation:[-90f,0f]}}", x + 2);
            let base = start(d, x, &["difficulty easy", &shulker, &boat]).await?;
            board(d, ":oak_boat").await?;
            d.ticks(20).await?;
            let heights = peek(d, vehicle_y).await?;
            let off = get_off(d).await?;
            Ok((base, verdict(heights, top, 1.0).and(check(off, || "still seated".into()))))
        }
        "horse_lid_push" => {
            let saddle = "equipment:{saddle:{id:\"minecraft:saddle\",count:1}}";
            let horse = format!("summon horse {}.5 {top} 0.5 {{NoAI:1b,Tame:1b,Rotation:[-90f,0f],{saddle}}}", x + 2);
            let base = start(d, x, &[&shulker_box, &horse]).await?;
            board(d, ":horse").await?;
            d.ticks(20).await?;
            let heights = open_box(d, [x + 2, lid, 0], vehicle_y).await?;
            let off = get_off(d).await?;
            Ok((base, verdict(heights, top, 0.5).and(check(off, || "still seated".into()))))
        }
        n => Err(format!("no drill {n}")),
    }
}
