//! Does an [`Act`] with the bot's own actions.

use std::time::Instant;

use acacia_physics::BlockPos;

use super::Act;
use crate::act_timing::Phase;
use crate::interact::{block_distance, Face, BLOCK_REACH};
use crate::items::SlotRef;
use crate::signs::SignSide;
use crate::{ActionError, Bot};

/// Faces to click, in preference order: placing on top of the block below first.
const AGAINST: [Face; 6] = [Face::Up, Face::North, Face::South, Face::West, Face::East, Face::Down];

/// `human`: the process drives the Human profile, so item switches pause as a player's do.
pub(super) async fn perform(bot: &mut Bot, act: &Act, human: bool) -> Result<(), ActionError> {
    match act {
        Act::Fill { pos, blocks } => fill(bot, *pos, blocks, human).await,
        Act::Place { item, against, face, click } => {
            let slot = bot.find_item(item).ok_or_else(|| ActionError::NotPossible(format!("no {item} to place")))?;
            hand(bot, slot, human).await?;
            bot.place_block_at(*against, *face, *click).await
        }
        Act::Break { pos } => {
            if bot.block_at(*pos).is_some_and(|(_, s)| s.is_air() || s.is_liquid()) {
                return Ok(());
            }
            if let Some(slot) = bot.best_tool_for(*pos) {
                hand(bot, slot, human).await?;
            }
            bot.break_block(*pos).await
        }
        Act::Restock { station, items } => {
            let t = Instant::now();
            let r = restock(bot, *station, items).await;
            bot.act_timing.add(Phase::Restock, t.elapsed());
            r
        }
        Act::Use { pos, item, face } => {
            if let Some(item) = item {
                let slot = bot.find_item(item).ok_or_else(|| ActionError::NotPossible(format!("no {item} to use")))?;
                hand(bot, slot, human).await?;
            }
            bot.use_confirmed(*pos, *face).await
        }
        Act::Sign { pos, front, text } => bot.write_sign(*pos, if *front { SignSide::Front } else { SignSide::Back }, text).await,
    }
}

/// Sends an [`Act::Place`] without awaiting its echo (Optimal profile); returns the target cell, whose outcome
/// [`Bot::settle_placements`] gives on a later tick.
pub(super) async fn send_place(bot: &mut Bot, act: &Act) -> Result<BlockPos, ActionError> {
    let Act::Place { item, against, face, click } = act else { unreachable!("placements only") };
    let slot = bot.find_item(item).ok_or_else(|| ActionError::NotPossible(format!("no {item} to place")))?;
    hand(bot, slot, false).await?;
    bot.place_sent(*against, *face, *click).await?;
    Ok(face.adjacent(*against))
}

/// Brings `slot` to the hand, timed as a switch when the held item changed (see [`crate::act_timing`]).
async fn hand(bot: &mut Bot, slot: SlotRef, human: bool) -> Result<(), ActionError> {
    let stack = slot.stack(&bot.state).filter(|s| !s.is_empty()).cloned();
    let stack = stack.ok_or_else(|| ActionError::NotPossible(format!("{slot:?} is empty")))?;
    let (before, t) = (bot.state.inventory.held().network_id, Instant::now());
    let r = bot.equip_hand_paced(slot, &stack, human).await;
    let phase = if bot.state.inventory.held().network_id != before { Phase::EquipSwitch } else { Phase::EquipSame };
    bot.act_timing.add(phase, t.elapsed());
    r
}

/// Places the first of `blocks` held into `pos`; done once the server shows a block there.
async fn fill(bot: &mut Bot, pos: BlockPos, blocks: &[String], human: bool) -> Result<(), ActionError> {
    let solid = |bot: &Bot, p: BlockPos| bot.block_at(p).map(|(_, s)| !s.is_air() && !s.is_liquid());
    match solid(bot, pos) {
        Some(true) => return Ok(()),
        None => return Err(ActionError::NotPossible(format!("{pos:?} is not loaded"))),
        Some(false) => {}
    }
    let eye = bot.eye_position();
    let face = AGAINST.into_iter().find(|f| {
        let against = against(pos, *f);
        solid(bot, against) == Some(true)
            && !bot.block_at(against).is_some_and(|(_, s)| crate::interact::is_interactive(s))
            && block_distance(eye, against) <= BLOCK_REACH
            && bot.sees_face(against, *f)
    });
    let face = face.ok_or_else(|| ActionError::NotPossible(format!("nothing in view to place against around {pos:?} within reach")))?;
    let slot = blocks.iter().find_map(|b| bot.find_item(b));
    let slot = slot.ok_or_else(|| ActionError::NotPossible(format!("none of {blocks:?} to fill {pos:?} with")))?;
    hand(bot, slot, human).await?;
    bot.place_block(against(pos, face), face).await
}

/// Takes whole stacks of the wanted items until each count is covered or the inventory is full.
async fn restock(bot: &mut Bot, station: BlockPos, items: &[(String, u32)]) -> Result<(), ActionError> {
    let container = bot.open_container_at(station).await?;
    tracing::debug!(?station, slots = container.slots.len(), ?items, "restock opened");
    let mut want: Vec<(String, u32)> = items.to_vec();
    let mut result = Ok(());
    for (slot, stack) in container.slots.iter().enumerate() {
        let Some(name) = bot.state.items.name(stack.network_id).map(str::to_string) else { continue };
        let Some(w) = want.iter_mut().find(|(n, c)| *n == name && *c > 0) else { continue };
        let Ok(slot) = u8::try_from(slot) else { continue };
        let moved = bot.move_to_inventory(slot).await;
        tracing::debug!(slot, %name, count = stack.count, ?moved, "restock take");
        match moved {
            Ok(_) => w.1 = w.1.saturating_sub(u32::from(stack.count)),
            Err(ActionError::NotPossible(_)) => break,
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    let closed = bot.close_container().await;
    result.and(closed)
}

/// The block whose `face` touches `pos`.
fn against(pos: BlockPos, face: Face) -> BlockPos {
    let [dx, dy, dz] = face.offset();
    [pos[0] - dx, pos[1] - dy, pos[2] - dz]
}
