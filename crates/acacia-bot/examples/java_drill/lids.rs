//! Lid drills: the bot stands on a shulker's lid as it opens and must ride it up. Grim tolerates a missed push near
//! shulkers, so each checks the rise itself. `shulker_push`: a NoAI shulker on the floor peeks fully (its lid rises a
//! block over 20 ticks), then closes. `shulker_box_push`: the bot opens the shulker box it stands on (the lid rises
//! half a block and holds it up), then closes it.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["shulker_push", "shulker_box_push"];

/// The highest and the last feet over `n` ticks standing still.
async fn watch(d: &mut Drill, n: u32) -> Result<(f64, f64), String> {
    let mut top = f64::MIN;
    for _ in 0..n {
        d.ticks(1).await?;
        top = top.max(d.feet()[1]);
    }
    Ok((top, d.feet()[1]))
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let (lid, top) = (FLOOR + 1, f64::from(FLOOR + 2));
    let stand = format!("tp @s {}.5 {top} 0.5 -90 0", x + 2);
    match name {
        "shulker_push" => {
            let shulker = format!("summon shulker {} {lid} 0 {{NoAI:1b,Peek:0b,AttachFace:0b}}", x + 2);
            let base = start(d, x, &["difficulty easy", &shulker, &stand]).await?;
            let at = "@e[type=shulker,limit=1,sort=nearest]";
            d.bot.command(&format!("data merge entity {at} {{Peek:100b}}"));
            let (high, open) = watch(d, 40).await?;
            d.bot.command(&format!("data merge entity {at} {{Peek:0b}}"));
            let (_, shut) = watch(d, 40).await?;
            d.bot.command("difficulty peaceful");
            d.walk(20, 1.0, false, false).await?;
            let ok = high > top + 0.95 && (open - (top + 1.0)).abs() < 1e-3 && (shut - top).abs() < 1e-3;
            Ok((base, check(ok, || format!("rose to {:.3}, rested at {:.3} open and {:.3} shut", high - top, open - top, shut - top))))
        }
        "shulker_box_push" => {
            let shulker_box = format!("setblock {} {lid} 0 shulker_box[facing=up]", x + 2);
            let base = start(d, x, &[&shulker_box, &stand]).await?;
            d.bot.open_container_at([x + 2, lid, 0]).await.map_err(|e| format!("opening: {e}"))?;
            let (high, open) = watch(d, 30).await?;
            d.bot.close_container().await.map_err(|e| format!("closing: {e}"))?;
            let (_, shut) = watch(d, 30).await?;
            d.walk(20, 1.0, false, false).await?;
            let ok = high > top + 0.45 && (open - (top + 0.5)).abs() < 1e-3 && (shut - top).abs() < 1e-3;
            Ok((base, check(ok, || format!("rose to {:.3}, rested at {:.3} open and {:.3} shut", high - top, open - top, shut - top))))
        }
        n => Err(format!("no drill {n}")),
    }
}
