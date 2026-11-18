//! `fish`: a pool a few blocks ahead, a Lure III rod in hand; the bot casts, waits for its bobber's bite, reels in
//! and checks the catch reached the inventory.

use std::time::Duration;

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["fish"];

/// Lure III takes 15 s off vanilla's 5-30 s wait.
const BITE_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let pool = format!("fill {} {} -2 {} {FLOOR} 2 water", x + 3, FLOOR - 2, x + 8);
    let rod = "give @s fishing_rod[enchantments={lure:3}]";
    let base = start(d, x, &[&pool, rod]).await?;
    d.bot.look_at([x as f32 + 5.5, FLOOR as f32 + 1.0, 0.5]).await.map_err(|e| format!("look: {e}"))?;
    let caught = d.bot.fish(BITE_TIMEOUT).await;
    d.ticks(20).await?;
    let name = caught.as_ref().ok().and_then(Option::as_ref).and_then(|s| d.bot.state().items.name(s.network_id)).map(str::to_owned);
    let hook_gone = d.bot.state().fishing.hook.is_none();
    Ok((base, check(name.is_some() && hook_gone, || format!("fish {caught:?} ({name:?}), hook gone {hook_gone}"))))
}
