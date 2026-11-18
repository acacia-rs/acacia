//! Riptide drills, holding a Riptide III trident looking up and east, then releasing: `riptide` holds 15 ticks in a
//! walled pool and must fly clear of the water; `riptide_short` lets go after 5 ticks, which must do nothing.
//! `rain_riptide` launches on dry plains in the rain; `rain_roofed` stands under glass in the rain, where the
//! heightmap and sky light say it's dry, and must not launch. `riptide_armor_stand` launches from the pool into
//! armour stands, which are living: the spin bounces back off them, so the player drifts back west.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["riptide", "riptide_short", "rain_riptide", "rain_roofed", "riptide_armor_stand"];

/// `/weather rain` ramps the level 0.01 a tick; it rains above 0.2.
const RAIN_WAIT_TICKS: u32 = 200;

/// The setup commands, and the fill that clears the course afterwards.
fn course(name: &str, x: i32) -> (Vec<String>, Option<String>) {
    let surface = FLOOR + 1;
    let (mut setup, clear) = match name {
        "riptide" | "riptide_short" | "riptide_armor_stand" => {
            let mut pool = vec![
                format!("fill {} {} -3 {} {surface} 3 glass", x - 2, FLOOR - 2, x + 4),
                format!("fill {} {} -2 {} {surface} 2 water", x - 1, FLOOR - 2, x + 3),
            ];
            if name == "riptide_armor_stand" {
                // A column of floating stands across the launch: armour stands are living, so the spin bounces off.
                for dy in [1, 3, 5] {
                    pool.push(format!("summon armor_stand {}.5 {} 0.5 {{NoGravity:1b,Invulnerable:1b}}", x + 2, surface + dy));
                }
            }
            (pool, Some(format!("fill {} {} -3 {} {surface} 3 air", x - 2, FLOOR - 2, x + 4)))
        }
        "rain_roofed" => {
            let roof = |block: &str| format!("fill {} {} -3 {} {} 3 {block}", x - 2, surface + 3, x + 4, surface + 3);
            (vec![roof("glass")], Some(roof("air")))
        }
        _ => (Vec::new(), None),
    };
    setup.push(r#"give @s minecraft:trident[enchantments={"minecraft:riptide":3}]"#.to_owned());
    if name.starts_with("rain") {
        setup.push("weather rain".to_owned());
    }
    (setup, clear)
}

fn raining(d: &Drill) -> bool {
    d.bot.java().is_some_and(|j| j.client().world.is_raining())
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let (setup, clear) = course(name, x);
    let setup: Vec<&str> = setup.iter().map(String::as_str).collect();
    let base = start(d, x, &setup).await?;
    if name.starts_with("rain") {
        let mut waited = 0;
        while !raining(d) && waited < RAIN_WAIT_TICKS {
            d.ticks(1).await?;
            waited += 1;
        }
    }
    let rain = raining(d);
    let in_rain = d.bot.java().is_some_and(|j| j.client().in_rain());
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, -40.0);
    }
    d.ticks(5).await?;
    d.bot.use_item();
    d.ticks(if name == "riptide_short" { 5 } else { 15 }).await?;
    d.bot.release_item();
    let stands = d.bot.state().entities.iter().filter(|e| e.kind.ends_with(":armor_stand")).count();
    let start_at = d.feet();
    let (mut top, mut far) = (start_at[1], start_at[0]);
    for _ in 0..60 {
        d.ticks(1).await?;
        let f = d.feet();
        (top, far) = (top.max(f[1]), far.max(f[0]));
    }
    let (rise, run, back) = (top - start_at[1], far - start_at[0], far - d.feet()[0]);
    let flew = || format!("rose {rise:.2}, ran {run:.2}, back {back:.2}, raining {rain}, in rain {in_rain}, {stands} stands known");
    let result = match name {
        "riptide" | "rain_riptide" => check(rise > 3.0 && run > 3.0, flew),
        // Only the bounce's reversed delta carries the player back west; the peak tick itself may go unsampled.
        "riptide_armor_stand" => check(back > 1.0 && run < 3.0, flew),
        _ => check(rise < 1.0 && run < 1.0, flew),
    };
    if name.starts_with("rain") {
        d.bot.command("weather clear");
    }
    if let Some(clear) = clear {
        d.bot.command(&clear);
    }
    d.ticks(5).await?;
    Ok((base, result))
}
