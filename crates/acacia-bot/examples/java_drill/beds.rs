//! Beds. `sleep`: at night the bot lies down in a bed two blocks ahead, stays asleep a while, presses "leave bed",
//! stands up where the server puts it and walks on. `sleep_day`: by day the server refuses with its no-sleep text.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["sleep", "sleep_day"];

const ASLEEP_TICKS: u32 = 40;

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let (foot, head) = (x + 2, x + 3);
    let time = if name == "sleep" { "time set 18000" } else { "time set 1000" };
    let foot_cmd = format!("setblock {foot} {surface} 0 red_bed[facing=east,part=foot]");
    let head_cmd = format!("setblock {head} {surface} 0 red_bed[facing=east,part=head]");
    let base = start(d, x, &[time, &head_cmd, &foot_cmd]).await?;
    let slept = d.bot.sleep([foot, surface, 0]).await;
    if name == "sleep_day" {
        let refused = matches!(&slept, Err(acacia_bot::ActionError::Rejected(m)) if m.contains("block.minecraft.bed.no_sleep"));
        d.ticks(20).await?;
        return Ok((base, check(refused, || format!("by day: {slept:?}"))));
    }
    slept.map_err(|e| format!("sleep: {e}"))?;
    d.ticks(ASLEEP_TICKS).await?;
    let (asleep, lying) = (d.bot.is_sleeping(), d.feet());
    // `BedBlock.useWithoutItem` sleeps in the head piece whichever piece was clicked.
    let in_bed = (lying[0] - (f64::from(head) + 0.5)).abs() < 0.01 && lying[1] > f64::from(surface) && lying[1] < f64::from(surface) + 0.75;
    d.bot.wake().await.map_err(|e| format!("wake: {e}"))?;
    d.ticks(20).await?;
    let stood = d.feet();
    d.walk(30, 1.0, false, false).await?;
    let walked = (d.feet()[0] - stood[0]).abs() + (d.feet()[2] - stood[2]).abs();
    let ok = asleep && in_bed && !d.bot.is_sleeping() && walked > 2.0;
    Ok((base, check(ok, || format!("asleep {asleep}, lying at {lying:.3?}, stood at {stood:.3?}, walked {walked:.2}"))))
}
