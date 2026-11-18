//! `hold_mine`: the bot looks at a dirt block two blocks ahead and holds attack on it ([`acacia_bot::Bot::start_mining`])
//! until it breaks, then mines the one above it, lets go halfway, checks it stands, and mines it again from scratch.
//! Grim's FastBreak judges the timing.

use acacia_bot::interact::Face;

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["hold_mine"];

/// Dirt by hand: 0.5 hardness, 1/15 a tick after the start tick, 16 bot ticks in all. The drill counts its own waits,
/// and a late tick's catch-up runs two bot ticks in one, so a few fewer show; Grim's FastBreak judges the real timing.
const DIRT_TICKS: std::ops::RangeInclusive<u32> = 12..=19;

/// Ticks until the block at `pos` is air, holding attack on it; `None` after `limit`.
async fn mine(d: &mut Drill, pos: [i32; 3], limit: u32) -> Result<Option<u32>, String> {
    d.bot.look_at([pos[0] as f32 + 0.5, pos[1] as f32 + 0.5, pos[2] as f32 + 0.5]).await.map_err(|e| format!("look: {e}"))?;
    d.bot.start_mining(pos, Face::West).map_err(|e| format!("start mining: {e}"))?;
    for t in 1..=limit {
        d.ticks(1).await?;
        if d.bot.block_name(pos) == Some("minecraft:air") {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let (low, high) = ([x + 2, surface, 0], [x + 2, surface + 1, 0]);
    let place = |[x, y, z]: [i32; 3]| format!("setblock {x} {y} {z} dirt");
    let (a, b) = (place(low), place(high));
    let base = start(d, x, &[&a, &b]).await?;
    let first = mine(d, low, 40).await?;
    d.bot.look_at([high[0] as f32 + 0.5, high[1] as f32 + 0.5, 0.5]).await.map_err(|e| format!("look: {e}"))?;
    d.bot.start_mining(high, Face::West).map_err(|e| format!("start mining: {e}"))?;
    d.ticks(6).await?;
    d.bot.stop_mining();
    d.ticks(10).await?;
    let survived = d.bot.block_name(high) == Some("minecraft:dirt");
    let second = mine(d, high, 40).await?;
    d.ticks(10).await?;
    let ok = first.is_some_and(|t| DIRT_TICKS.contains(&t)) && survived && second.is_some_and(|t| DIRT_TICKS.contains(&t));
    Ok((base, check(ok, || format!("first {first:?} ticks, let go and survived {survived}, again {second:?} ticks"))))
}
