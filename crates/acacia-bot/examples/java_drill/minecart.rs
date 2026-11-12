//! `minecart`: the bot boards a cart on a rail line east (a powered run, a one-block hill up and down, then unpowered
//! powered rails that halt it), pushes off with forward, rides to the stop and gets off. The server moves the cart;
//! the passenger only sends input and look, so Grim judges the boarding, the input and the dismount.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["minecart"];

const RIDE_TICKS: u32 = 200;
const PUSH_TICKS: u32 = 20;

fn track(x: i32) -> Vec<String> {
    let (s, top) = (FLOOR + 1, FLOOR + 2);
    let rail = |from: i32, to: i32, y: i32, block: &str| format!("fill {} {y} 0 {} {y} 0 {block}", x + from, x + to);
    vec![
        format!("fill {} {FLOOR} 0 {} {FLOOR} 0 redstone_block", x + 3, x + 8),
        format!("fill {} {s} 0 {} {s} 0 stone", x + 14, x + 18),
        format!("setblock {} {s} 0 stone", x + 29),
        rail(2, 2, s, "rail[shape=east_west]"),
        rail(3, 8, s, "powered_rail[shape=east_west]"),
        rail(9, 12, s, "rail[shape=east_west]"),
        rail(13, 13, s, "rail[shape=ascending_east]"),
        rail(14, 18, top, "rail[shape=east_west]"),
        rail(19, 19, s, "rail[shape=ascending_west]"),
        rail(20, 23, s, "rail[shape=east_west]"),
        rail(24, 28, s, "powered_rail[shape=east_west]"),
    ]
}

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let mut setup = track(x);
    setup.push(format!("summon minecraft:minecart {}.5 {surface} 0.5", x + 2));
    let setup: Vec<&str> = setup.iter().map(String::as_str).collect();
    let base = start(d, x, &setup).await?;
    let cart = d.bot.state().entities.iter().find(|e| e.kind.ends_with(":minecart")).map(|e| e.runtime_id).ok_or("no minecart tracked")?;
    d.bot.mount(cart).await.map_err(|e| format!("boarding: {e}"))?;
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, 0.0);
    }
    let from = d.feet();
    let (mut top, mut stopped, mut last) = (from[1], 0, from);
    for t in 0..RIDE_TICKS {
        d.walk(1, if t < PUSH_TICKS { 1.0 } else { 0.0 }, false, false).await?;
        let f = d.feet();
        top = top.max(f[1]);
        stopped = if (f[0] - last[0]).abs() < 1e-4 && t > PUSH_TICKS { stopped + 1 } else { 0 };
        last = f;
    }
    let (ran, rose) = (last[0] - from[0], top - from[1]);
    let riding = d.bot.vehicle().is_some();
    d.bot.dismount().await.map_err(|e| format!("dismount: {e}"))?;
    d.ticks(40).await?;
    let off = d.bot.vehicle().is_none();
    let ok = riding && off && ran > 20.0 && rose > 0.9 && stopped > 10;
    Ok((base, check(ok, || format!("ran {ran:.2}, rose {rose:.2}, still for {stopped} ticks, seated {riding}, off {off}"))))
}
