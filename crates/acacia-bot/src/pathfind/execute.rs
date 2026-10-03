//! Performs a move's [`Work`](super::Work) with the bot's own actions: [`Bot::break_block`] with
//! the best tool, [`Bot::place_block`] with a scaffold block, [`Bot::use_item_on_block`] on doors.

use std::time::Duration;

use acacia_physics::BlockPos;

use super::follow::Sense;
use super::maneuver;
use super::moves::MoveKind;
use super::work::Step;
use super::PathNode;
use crate::interact::{facing_face, Face};
use crate::movement::Controls;
use crate::survival::Destination;
use crate::{ActionError, Bot};

/// Ticks to sneak to an edge before placing anyway.
const EDGE_TICKS: u32 = 30;
/// Ticks for a pillar jump to clear the cell.
const RISE_TICKS: u32 = 12;
const DOOR_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) async fn perform(bot: &mut Bot, node: &PathNode, scaffold: &[String]) -> Result<(), ActionError> {
    bot.stop_pathing();
    for step in node.work.steps() {
        match step {
            Step::Break(pos) => dig(bot, pos).await?,
            Step::Door { pos, open } => door(bot, pos, open).await?,
            Step::Place { against, face } => {
                let placed = place(bot, node.kind, against, face, scaffold).await;
                if let Some(c) = bot.controls() {
                    c.stop();
                    c.sneak = false;
                }
                placed?;
            }
        }
    }
    Ok(())
}

async fn dig(bot: &mut Bot, pos: BlockPos) -> Result<(), ActionError> {
    if bot.block_at(pos).is_some_and(|(_, s)| s.is_air() || s.is_liquid()) {
        return Ok(());
    }
    bot.equip_best_tool(pos).await?;
    bot.break_block(pos).await
}

async fn door(bot: &mut Bot, pos: BlockPos, open: bool) -> Result<(), ActionError> {
    let want = if open { "1" } else { "0" };
    let state = |bot: &Bot| bot.block_at(pos).and_then(|(_, s)| s.property("open_bit"));
    match state(bot) {
        Some(v) if v == want => return Ok(()),
        None => return Err(ActionError::NotPossible(format!("no door at {pos:?}"))),
        Some(_) => {}
    }
    bot.use_item_on_block(pos, facing_face(bot.eye_position(), pos)).await?;
    match bot.wait_until(DOOR_TIMEOUT, |bot, _| (state(bot) == Some(want)).then_some(())).await {
        Err(ActionError::Timeout) => Err(ActionError::Rejected(format!("the door at {pos:?} did not move"))),
        other => other,
    }
}

async fn place(bot: &mut Bot, kind: MoveKind, against: BlockPos, face: Face, scaffold: &[String]) -> Result<(), ActionError> {
    let target = face.adjacent(against);
    if bot.block_at(target).is_some_and(|(_, s)| !s.is_air() && !s.is_liquid()) {
        return Ok(());
    }
    let slot = bot.scaffold_slot(scaffold).ok_or_else(|| ActionError::NotPossible("no scaffold blocks left".into()))?;
    bot.equip(slot, Destination::Hand).await?;
    if kind == MoveKind::Pillar {
        bot.look_at([against[0] as f32 + 0.5, (against[1] + 1) as f32, against[2] as f32 + 0.5]).await?;
        if !maneuver_until(bot, RISE_TICKS, |s, c| maneuver::rise(s, target, c)).await? {
            return Err(ActionError::NotPossible(format!("could not jump above {target:?}")));
        }
    } else {
        let [dx, _, dz] = face.offset();
        let from = [target[0] - dx, target[1] + 1, target[2] - dz];
        maneuver_until(bot, EDGE_TICKS, |s, c| maneuver::to_edge(s, from, (dx, dz), c)).await?;
    }
    bot.place_block(against, face).await
}

/// Lets ticks pass with `step` setting the controls until it reports done (true) or `ticks` run out.
async fn maneuver_until(bot: &mut Bot, ticks: u32, mut step: impl FnMut(&Sense, &mut Controls) -> bool) -> Result<bool, ActionError> {
    for _ in 0..ticks {
        let Some(sense) = sense(bot) else { return Err(ActionError::NotPossible("no physics position".into())) };
        let Some(controls) = bot.controls() else { return Ok(false) };
        if step(&sense, controls) {
            return Ok(true);
        }
        bot.next_tick(|_, _| false).await?;
    }
    Ok(false)
}

fn sense(bot: &Bot) -> Option<Sense> {
    let m = bot.movement.as_ref()?;
    let pos = m.position()?;
    let feet = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
    let in_water = bot.block_at(feet).is_some_and(|(_, s)| s.is_water());
    Some(Sense { pos, on_ground: m.on_ground(), in_water })
}
