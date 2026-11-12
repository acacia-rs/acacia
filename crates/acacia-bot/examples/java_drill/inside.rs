//! Blocks that act on a body inside them: `cobweb` and `berry_bush` slow a walk east, `cauldron` hops through
//! cauldrons (water at each level, lava, powder snow) and over tripwire, and `powder_snow_fall` drops 6 blocks
//! onto powder snow, which holds a falling body on its lower 0.9 until the fall is over, then lets it sink.
//! `powder_snow_fall` isn't in the default list: Grim sticks any body in the snow's cell, so it flags the landing a
//! real 26.3 client makes too (replay trace `snowfall`).

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["cobweb", "berry_bush", "cauldron", "powder_snow_fall"];

/// A walk east from x0 covers ~13 blocks in 60 ticks.
const WALK_TICKS: u32 = 60;

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let s = FLOOR + 1;
    let x0 = f64::from(x) + 0.5;
    match name {
        "cobweb" => {
            let web = format!("fill {} {s} -1 {} {} 1 cobweb", x + 3, x + 8, s + 1);
            let base = start(d, x, &[&web]).await?;
            d.walk(WALK_TICKS, 1.0, false, false).await?;
            let moved = d.feet()[0] - x0;
            d.bot.command(&format!("fill {} {s} -1 {} {} 1 air", x + 3, x + 8, s + 1));
            Ok((base, check((2.0..6.0).contains(&moved), || format!("moved {moved:.2}"))))
        }
        "berry_bush" => {
            let bushes = format!("fill {} {s} -1 {} {s} 1 sweet_berry_bush[age=0]", x + 1, x + 20);
            let base = start(d, x, &[&bushes]).await?;
            d.walk(WALK_TICKS, 1.0, false, false).await?;
            let moved = d.feet()[0] - x0;
            Ok((base, check((2.0..9.0).contains(&moved), || format!("moved {moved:.2}"))))
        }
        "cauldron" => {
            let row = [
                (3, "water_cauldron[level=1]"),
                (6, "water_cauldron[level=2]"),
                (9, "water_cauldron[level=3]"),
                (12, "lava_cauldron"),
                (15, "powder_snow_cauldron[level=3]"),
                (18, "tripwire"),
                (19, "tripwire[attached=true]"),
            ];
            let mut setup: Vec<String> = row.iter().map(|(dx, block)| format!("setblock {} {s} 0 {block}", x + dx)).collect();
            setup.push("effect give @s fire_resistance 60 0 true".into());
            let setup: Vec<&str> = setup.iter().map(String::as_str).collect();
            let base = start(d, x, &setup).await?;
            d.walk(180, 1.0, true, false).await?;
            d.ticks(20).await?;
            let moved = d.feet()[0] - x0;
            Ok((base, check(moved > 20.0, || format!("moved {moved:.2}"))))
        }
        "powder_snow_fall" => {
            let top = s + 6;
            let setup = [
                format!("fill {} {} -1 {} {} 1 powder_snow", x + 2, s, x + 4, s + 1),
                format!("setblock {} {} 0 glass", x + 3, top - 1),
                format!("tp @s {}.5 {top} 0.5", x + 3),
            ];
            let setup: Vec<&str> = setup.iter().map(String::as_str).collect();
            let base = start(d, x, &setup).await?;
            d.bot.command(&format!("setblock {} {} 0 air", x + 3, top - 1));
            let (mut held, mut low) = (f64::MIN, f64::MAX);
            for _ in 0..60 {
                d.ticks(1).await?;
                let feet = d.feet()[1];
                if (feet - (f64::from(s + 1) + 0.9)).abs() < 1e-6 {
                    held = feet;
                }
                low = low.min(feet);
            }
            d.bot.command(&format!("fill {} {} -1 {} {} 1 air", x + 2, s, x + 4, s + 1));
            d.ticks(10).await?;
            let ok = held > f64::MIN && low < f64::from(s + 1);
            Ok((base, check(ok, || format!("held on top {}, lowest {low:.3}", held > f64::MIN))))
        }
        _ => Err(format!("no inside drill {name}")),
    }
}
