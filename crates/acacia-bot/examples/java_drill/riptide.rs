//! Riptide drills in a walled pool: `riptide` holds a Riptide III trident 15 ticks looking up and east, releases,
//! and must fly clear of the water; `riptide_short` lets go after 5 ticks, which must do nothing.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["riptide", "riptide_short"];

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let pool = [
        format!("fill {} {} -3 {} {surface} 3 glass", x - 2, FLOOR - 2, x + 4),
        format!("fill {} {} -2 {} {surface} 2 water", x - 1, FLOOR - 2, x + 3),
        r#"give @s minecraft:trident[enchantments={"minecraft:riptide":3}]"#.to_owned(),
    ];
    let setup: Vec<&str> = pool.iter().map(String::as_str).collect();
    let base = start(d, x, &setup).await?;
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, -40.0);
    }
    d.ticks(5).await?;
    let hold = if name == "riptide" { 15 } else { 5 };
    d.bot.use_item();
    d.ticks(hold).await?;
    d.bot.release_item();
    let start_at = d.feet();
    let (mut top, mut far) = (start_at[1], start_at[0]);
    for _ in 0..60 {
        d.ticks(1).await?;
        let f = d.feet();
        (top, far) = (top.max(f[1]), far.max(f[0]));
    }
    let (rise, run) = (top - start_at[1], far - start_at[0]);
    let launched = rise > 3.0 && run > 3.0;
    let result = if name == "riptide" { check(launched, || format!("rose {rise:.2}, ran {run:.2}")) } else { check(rise < 1.0 && run < 1.0, || format!("short hold launched: rose {rise:.2}, ran {run:.2}")) };
    d.bot.command(&format!("fill {} {} -3 {} {surface} 3 air", x - 2, FLOOR - 2, x + 4));
    d.ticks(5).await?;
    Ok((base, result))
}
