//! Drills where another entity's size decides a push: the driller walks east past NoAI mobs whose boxes only touch
//! its own at their true size. Pushing at a wrong size moves the player where Grim doesn't, so 0 alerts is the test.
//! `baby_push`: a baby villager 0.5 to the side (an adult's box would push), then one 0.2 to the side (pushes).
//! `slime_push`: a size-4 slime 1.2 to the side, which only its scaled box reaches.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["baby_push", "slime_push"];

/// Walks east `n` ticks; the largest |z - 0.5| on the way, and z at the end.
async fn walk_by(d: &mut Drill, n: u32) -> Result<(f64, f64), String> {
    let mut off: f64 = 0.0;
    for _ in 0..n {
        d.walk(1, 1.0, false, false).await?;
        off = off.max((d.feet()[2] - 0.5).abs());
    }
    Ok((off, d.feet()[2]))
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let y = FLOOR + 1;
    match name {
        "baby_push" => {
            let baby = |dx: i32, z: f64| format!("summon villager {}.5 {y} {z} {{NoAI:1b,Age:-24000,Rotation:[90f,0f]}}", x + dx);
            let base = start(d, x, &[&baby(4, 1.0)]).await?;
            let (passed, _) = walk_by(d, 30).await?;
            d.bot.command(&baby(10, d.feet()[2] + 0.2));
            d.ticks(10).await?;
            let before = d.feet()[2];
            walk_by(d, 30).await?;
            let shoved = before - d.feet()[2];
            Ok((base, check(passed == 0.0 && shoved > 0.01, || format!("side-step past the first {passed:.4}, shoved {shoved:.3}"))))
        }
        "slime_push" => {
            let slime = format!("summon slime {}.5 {y} 1.7 {{NoAI:1b,Size:3}}", x + 5);
            let base = start(d, x, &["difficulty easy", &slime]).await?;
            let (_, z) = walk_by(d, 40).await?;
            d.bot.command("difficulty peaceful");
            let world = d.bot.java().map(|j| &j.client().world);
            let slime = acacia_java_proto::entity_types::ENTITY_TYPES.iter().position(|&n| n == "minecraft:slime").map(|k| k as i32);
            let seen: Vec<_> = world.iter().flat_map(|w| w.entities.values()).filter(|e| Some(e.kind) == slime && e.position[2].abs() < 4.0).map(|e| (e.position, e.dimensions())).collect();
            Ok((base, check(z < 0.45, || format!("z {z:.3}: the slime didn't push; slime {seen:?}"))))
        }
        n => Err(format!("no drill {n}")),
    }
}
