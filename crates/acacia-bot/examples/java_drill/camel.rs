//! `camel_dash`: the bot rides a saddled NoAI camel east, sprints (the passenger's sprint rules and the camel's sprint
//! bonus), stops, charges a dash from a standstill and lets it land; a second charge inside the dash cooldown fills no
//! bar and sends nothing. Then it gets off. The client drives the camel, so Grim judges its moves, the sprint and the dash.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

pub const NAMES: &[&str] = &["camel_dash"];

const CHARGE_TICKS: u32 = 10;

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let camel = format!("summon camel {}.5 {} 0.5 {{NoAI:1b,Rotation:[-90f,0f],equipment:{{saddle:{{id:\"minecraft:saddle\",count:1}}}}}}", x + 2, FLOOR + 1);
    let base = start(d, x, &[&camel]).await?;
    let id = d.bot.state().entities.iter().find(|e| e.kind.ends_with(":camel")).map(|e| e.runtime_id).ok_or("no camel tracked")?;
    d.bot.mount(id).await.map_err(|e| format!("boarding: {e}"))?;
    if let Some(c) = d.bot.controls() {
        (c.yaw, c.pitch) = (-90.0, 0.0);
    }
    d.ticks(10).await?;
    let from = d.bot.java_mount().map(|m| m.body.pos).ok_or("not driving the camel")?;
    d.walk(10, 1.0, false, false).await?;
    let mut sprinted = 0;
    for _ in 0..15 {
        d.walk(1, 1.0, false, true).await?;
        sprinted += u32::from(d.bot.java().is_some_and(|j| j.client().movement.player.sprinting));
    }
    d.walk(20, 0.0, false, false).await?;
    let still = d.bot.java().is_some_and(|j| !j.client().movement.player.sprinting);
    let before = d.bot.java_mount().map_or(0.0, |m| m.body.pos[0]);
    d.walk(CHARGE_TICKS, 0.0, true, false).await?;
    // The release reaches the camel on its next tick, before the rider's.
    let mut cooldown = 0;
    for _ in 0..30 {
        d.ticks(1).await?;
        cooldown = cooldown.max(d.bot.java_mount().map_or(0, |m| m.dash_cooldown));
    }
    let dashed = d.bot.java_mount().map_or(0.0, |m| m.body.pos[0]) - before;
    // A second charge inside the 55-tick cooldown: the bar doesn't fill and the camel stays put.
    d.walk(CHARGE_TICKS, 0.0, true, false).await?;
    d.ticks(20).await?;
    let to = d.bot.java_mount().map(|m| m.body.pos).ok_or("lost the camel")?;
    d.bot.dismount().await.map_err(|e| format!("dismount: {e}"))?;
    d.ticks(40).await?;
    let off = d.bot.vehicle().is_none();
    let rode = to[0] - from[0];
    let ok = sprinted > 10 && still && cooldown > 40 && dashed > 2.0 && off;
    Ok((base, check(ok, || format!("rode {rode:.2}, sprinted {sprinted}/15, sprint ended {still}, cooldown {cooldown}, dashed {dashed:.2}, off {off}"))))
}
