//! `rain_relit`: client relighting under rain. Under a stone roof the bot must be out of the rain; once the roof is
//! cleared it must be in rain on the tick after the client sees the air (its own sky light update, not the
//! server's), and a Riptide III release once the server has relit too must launch. Charging across the clear
//! instead (a 60-tick use with a command in it) drew a Grim Simulation setback on release; Grim's rain check
//! ignores roofs (`isInWaterOrRain`), so that isn't a light mismatch.

use acacia_registry::BlockState;

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["rain_relit"];

/// `/weather rain` ramps the level 0.01 a tick; it rains above 0.2.
const RAIN_WAIT_TICKS: u32 = 200;
/// `TridentItem.releaseUsing` needs 10 ticks of charge.
const CHARGE_TICKS: u32 = 15;
const SERVER_LIGHT_TICKS: u32 = 10;

fn client(d: &Drill) -> Option<&acacia_java_client::Client> {
    d.bot.java().map(|j| j.client())
}

pub async fn run(_name: &str, d: &mut Drill, x: i32) -> Outcome {
    let roof_y = FLOOR + 4;
    let roof = |block: &str| format!("fill {} {roof_y} -3 {} {roof_y} 3 {block}", x - 2, x + 4);
    let give = r#"give @s minecraft:trident[enchantments={"minecraft:riptide":3}]"#;
    let base = start(d, x, &[&roof("stone"), give, "weather rain"]).await?;
    let mut waited = 0;
    while !client(d).is_some_and(|c| c.world.is_raining()) && waited < RAIN_WAIT_TICKS {
        d.ticks(1).await?;
        waited += 1;
    }
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, -40.0);
    }
    d.ticks(5).await?;
    let roofed_rain = client(d).is_some_and(|c| c.in_rain());
    d.bot.command(&roof("air"));
    let above = d.feet().map(|v| v.floor() as i32);
    let cleared = |d: &Drill| client(d).and_then(|c| c.world.block([above[0], roof_y, above[2]])).and_then(BlockState::from_java_id).is_some_and(BlockState::is_air);
    let mut waited = 0;
    while !cleared(d) && waited < 40 {
        d.ticks(1).await?;
        waited += 1;
    }
    d.ticks(1).await?;
    let open_rain = client(d).is_some_and(|c| c.in_rain());
    // The server relights a few ticks behind, and a release before it has is a launch the server refuses.
    d.ticks(SERVER_LIGHT_TICKS).await?;
    d.bot.use_item();
    d.ticks(CHARGE_TICKS).await?;
    d.bot.release_item();
    let from = d.feet();
    let (mut top, mut far) = (from[1], from[0]);
    for _ in 0..60 {
        d.ticks(1).await?;
        let f = d.feet();
        (top, far) = (top.max(f[1]), far.max(f[0]));
    }
    let (rise, run) = (top - from[1], far - from[0]);
    d.bot.command("weather clear");
    d.ticks(5).await?;
    let what = || format!("roofed in rain {roofed_rain}; cleared after {waited} ticks: in rain {open_rain}, rose {rise:.2} ran {run:.2}");
    Ok((base, check(!roofed_rain && open_rain && rise > 3.0 && run > 3.0, what)))
}
