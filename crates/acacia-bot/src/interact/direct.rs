//! Direct control for callers that aim themselves (a player at the keyboard): hold-to-mine advanced
//! one tick at a time, and clicks that neither turn the bot nor wait. The aim is whatever
//! [`crate::movement::Controls`] carried last tick.

use acacia_client::proto::types::{Action, GameMode};
use acacia_physics::BlockPos;
use acacia_world::BlockState;

use super::break_time::break_ticks;
use super::geometry::{block_distance, BLOCK_REACH};
use super::wire::{self, SwingSource};
use super::Face;
use crate::{ActionError, Bot};

/// Vanilla keeps swinging while mining; one swing lasts 6 ticks.
const SWING_EVERY: u32 = 5;

/// A block being mined through [`Bot::start_mining`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Mining {
    pos: BlockPos,
    face: Face,
    /// Break time in ticks, fixed at the start.
    ticks: u32,
    done: u32,
    creative: bool,
}

/// Items whose `canDestroyInCreative` is false: BDS destroys nothing with them in creative.
fn cannot_destroy_in_creative(name: &str) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    name.ends_with("_sword") || name == "trident" || name == "mace"
}

/// Name fragments of blocks a click uses instead of building against: containers, doors, switches,
/// workstations. A sneaking player builds against them anyway.
const INTERACTIVE: &[&str] = &[
    "chest", "barrel", "shulker_box", "door", "gate", "button", "lever", "crafting_table", "furnace", "smoker",
    "anvil", "enchanting_table", "brewing_stand", "beacon", "hopper", "dispenser", "dropper", "loom", "stonecutter",
    "grindstone", "cartography_table", "smithing_table", "bed", "bell", "note_block", "repeater", "comparator",
    "daylight_detector", "cake", "campfire", "lectern", "composter", "jukebox", "respawn_anchor", "crafter",
];

/// Whether a plain right-click on this block uses it rather than placing the held block against it.
pub fn is_interactive(state: &BlockState) -> bool {
    let name = state.name.strip_prefix("minecraft:").unwrap_or(state.name);
    INTERACTIVE.iter().any(|part| name.contains(part)) || name.ends_with("_sign")
}

impl Bot {
    /// Mines `face` of the block at `pos` from the next tick on, as holding the attack button does.
    /// Another block aborts the one in progress; the same one carries on. Survival breaks take the
    /// vanilla time ([`break_ticks`]); creative ones are a single `StartBreak` and fail with a
    /// weapon held. Physics bots only.
    pub fn start_mining(&mut self, pos: BlockPos, face: Face) -> Result<(), ActionError> {
        if self.mining.is_some_and(|m| m.pos == pos) {
            return Ok(());
        }
        self.stop_mining();
        if !self.has_physics() {
            return Err(ActionError::NotPossible("block breaking needs a physics bot".into()));
        }
        let (_, state) = self.block_at(pos).ok_or_else(|| ActionError::NotPossible(format!("{pos:?} is not loaded")))?;
        if state.is_air() || state.is_liquid() {
            return Err(ActionError::NotPossible(format!("nothing to break at {pos:?}")));
        }
        let creative = self.state.player.game_mode == GameMode::Creative;
        if creative && self.held_name().is_some_and(cannot_destroy_in_creative) {
            return Err(ActionError::NotPossible("weapons break nothing in creative".into()));
        }
        let ticks = match self.state.player.game_mode {
            GameMode::Creative => 1,
            GameMode::Adventure | GameMode::Spectator => return Err(ActionError::NotPossible("game mode cannot break blocks".into())),
            _ => break_ticks(&state.mining, &self.break_conditions())
                .ok_or_else(|| ActionError::NotPossible(format!("{} cannot be broken", state.name)))?,
        };
        let distance = block_distance(self.eye_position(), pos);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("{pos:?} is {distance:.2} blocks away")));
        }
        self.mining = Some(Mining { pos, face, ticks, done: 0, creative });
        Ok(())
    }

    /// Stops mining (`AbortBreak` with the next tick) if a break is in progress.
    pub fn stop_mining(&mut self) {
        if let Some(m) = self.mining.take()
            && m.done > 0
            && let Some(movement) = self.movement.as_mut()
        {
            movement.pending_actions.push(wire::block_action(Action::AbortBreak, m.pos, m.face));
        }
    }

    /// Name of the tracked block at `pos` (`minecraft:air`...); `None` when it is not known.
    pub fn block_name(&self, pos: BlockPos) -> Option<&'static str> {
        self.block_at(pos).map(|(_, s)| s.name)
    }

    /// The block being mined and how far along (0 to 1), for a crack overlay.
    pub fn mining_progress(&self) -> Option<(BlockPos, f32)> {
        self.mining.map(|m| (m.pos, m.done as f32 / m.ticks as f32))
    }

    /// One tick of [`Bot::start_mining`]: the same block actions [`Bot::break_block`] sends, ending
    /// with `PredictBreak`. The server's `UpdateBlock` then removes the block.
    pub(crate) fn tick_mining(&mut self) {
        let Some(mut m) = self.mining else { return };
        if self.block_at(m.pos).is_none_or(|(_, s)| s.is_air()) {
            self.mining = None;
            return;
        }
        let step = if m.done == 0 { Action::StartBreak } else { Action::ContinueBreak };
        let mut actions = vec![wire::block_action(step, m.pos, m.face)];
        // Creative: StartBreak alone destroys at once (BDS `GameMode::startDestroyBlock`); a
        // Predict there without a Continue before it would only roll the block back.
        if m.done + 1 >= m.ticks && !m.creative {
            actions.push(wire::block_action(Action::PredictBreak, m.pos, m.face));
        }
        if let Some(movement) = self.movement.as_mut() {
            movement.pending_actions.extend(actions);
        }
        if m.done % SWING_EVERY == 0 {
            self.swing_from(Some(SwingSource::Mine));
        }
        m.done += 1;
        self.mining = (m.done < m.ticks).then_some(m);
    }

    /// Right-clicks `face` of the block at `pos` now, with the current aim. A held block (or boat,
    /// minecart) is placed against it unless the block is one a click uses ([`is_interactive`]) and
    /// the player is not sneaking; anything else uses the block or the item on it.
    pub fn right_click_block(&mut self, pos: BlockPos, face: Face) -> Result<(), ActionError> {
        let sneaking = self.movement.as_ref().is_some_and(|m| m.controls.sneak);
        let held = self.state.inventory.held();
        let places = held.block_runtime_id != 0 || self.held_name().is_some_and(|n| n.ends_with("boat") || n.ends_with("minecart"));
        let interactive = self.block_at(pos).is_some_and(|(_, s)| is_interactive(s));
        let build = places && (sneaking || !interactive);
        self.click_block(pos, face, if build { SwingSource::Build } else { SwingSource::Interact })
    }
}
