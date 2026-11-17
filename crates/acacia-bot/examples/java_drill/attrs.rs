//! Attribute drills: each sets one player attribute with `/attribute` and runs a course only that value passes, then
//! resets it. `sprint_taps` toggles sprinting every few ticks, so the server's echoed sprint state (attributes and
//! shared flags) keeps arriving after the client has moved on.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &[
    "gravity",
    "jump_strength",
    "step_height",
    "scale_small",
    "scale_big",
    "sneaking_speed",
    "movement_efficiency",
    "friction",
    "air_drag",
    "sprint_taps",
];

fn attribute(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "gravity" => ("gravity", "0.04"),
        "jump_strength" => ("jump_strength", "0.8"),
        "step_height" => ("step_height", "1.1"),
        "scale_small" => ("scale", "0.5"),
        "scale_big" => ("scale", "2"),
        "sneaking_speed" => ("sneaking_speed", "1"),
        "movement_efficiency" => ("movement_efficiency", "1"),
        "friction" => ("friction_modifier", "0.5"),
        "air_drag" => ("air_drag_modifier", "2"),
        _ => return None,
    })
}

pub async fn run(name: &str, d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let x0 = f64::from(x) + 0.5;
    let set = attribute(name).map(|(a, v)| format!("attribute @s minecraft:{a} base set {v}"));
    let course = match name {
        "step_height" => vec![format!("fill {} {surface} -4 {} {surface} 4 stone", x + 4, x + 30)],
        "scale_small" => vec![format!("fill {} {} -4 {} {} 4 stone", x + 4, surface + 1, x + 10, surface + 1)],
        "scale_big" => vec![
            format!("fill {} {surface} -4 {} {} -1 stone", x + 4, x + 10, surface + 4),
            format!("fill {} {surface} 1 {} {} 4 stone", x + 4, x + 10, surface + 4),
        ],
        "movement_efficiency" => vec![format!("fill {} {FLOOR} -4 {} {FLOOR} 4 soul_sand", x - 2, x + 30)],
        _ => vec![],
    };
    let setup: Vec<&str> = course.iter().chain(set.iter()).map(String::as_str).collect();
    let base = start(d, x, &setup).await?;
    let result = match name {
        "gravity" | "jump_strength" => {
            let top = crate::drills::track_top(d, 60, true).await?;
            check(top > f64::from(surface) + 2.0, || format!("top {:.2}", top - f64::from(surface)))
        }
        "step_height" => {
            d.walk(40, 1.0, false, false).await?;
            let f = d.feet();
            check(f[1] >= f64::from(surface) + 1.0 && f[0] > x0 + 5.0, || format!("at {:.2} {:.2}", f[0] - x0, f[1]))
        }
        "scale_small" => {
            d.walk(80, 1.0, false, false).await?;
            let moved = d.feet()[0] - x0;
            check(moved > 11.0, || format!("moved {moved:.2}"))
        }
        "scale_big" => {
            d.walk(60, 1.0, false, true).await?;
            let moved = d.feet()[0] - x0;
            check(moved < 4.0, || format!("moved {moved:.2} into a corridor too narrow"))
        }
        "sneaking_speed" => {
            sneak_walk(d, 40).await?;
            let moved = d.feet()[0] - x0;
            check(moved > 6.0, || format!("moved {moved:.2}"))
        }
        "movement_efficiency" => {
            d.walk(40, 1.0, false, false).await?;
            let moved = d.feet()[0] - x0;
            check(moved > 6.0, || format!("moved {moved:.2}"))
        }
        "friction" | "air_drag" => {
            d.walk(30, 1.0, true, true).await?;
            d.ticks(30).await?;
            Ok(())
        }
        "sprint_taps" => {
            for _ in 0..25 {
                d.walk(3, 1.0, false, true).await?;
                d.walk(1, 0.0, false, false).await?;
            }
            d.ticks(20).await?;
            Ok(())
        }
        _ => unreachable!(),
    };
    if let Some((a, _)) = attribute(name) {
        d.bot.command(&format!("attribute @s minecraft:{a} base reset"));
        d.ticks(5).await?;
    }
    Ok((base, result))
}

async fn sneak_walk(d: &mut Drill, n: u32) -> Result<(), String> {
    for _ in 0..n {
        if let Some(c) = d.bot.controls() {
            (c.forward, c.sneak) = (1.0, true);
        }
        d.ticks(1).await?;
    }
    if let Some(c) = d.bot.controls() {
        (c.forward, c.sneak) = (0.0, false);
    }
    Ok(())
}
