//! Crafting and workstations: crafting grid and table, furnaces, brewing stand, enchanting table,
//! anvil, grindstone, stonecutter, smithing table, loom, cartography table, beacon and villager
//! trading.
//!
//! Each action opens the block (or trader), moves inputs in one click at a time with human pauses,
//! sends the craft as one `ItemStackRequest` (craft action, `CraftResultsDeprecated`, `Consume`s,
//! `Place` out of the created-output slot), moves leftovers back and closes. Request shapes and
//! their sources: docs/research/workstations.md. Planning is pure (`*_plan` functions) so tests
//! can check the encoding without a server.

mod anvil;
mod anvil_cost;
mod beacon;
mod brewing;
mod cartography;
mod craft;
mod craftable;
mod creative;
mod enchant;
mod enchants;
mod furnace;
mod grid;
mod grindstone;
mod hand;
mod loom;
mod pick;
mod repair;
mod smithing;
mod trade;

#[cfg(test)]
mod tests;

pub use beacon::BeaconEffect;
pub use craft::CraftMode;

use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use crate::human;
use crate::items::craft::Craft;
use crate::items::{Op, SlotRef};
use crate::state::{GameState, Inventory, ItemStack};
use crate::{ActionError, Bot};

impl Bot {
    /// Opens the block at `pos` and checks that it shows one of `kinds`.
    async fn open_station(&mut self, pos: BlockPos, kinds: &[WindowType]) -> Result<(), ActionError> {
        let open = self.open_container_at(pos).await?;
        if kinds.contains(&open.window_type) {
            return Ok(());
        }
        self.close_container().await?;
        Err(ActionError::NotPossible(format!("the block at {pos:?} opened a {:?} window", open.window_type)))
    }

    /// [`Bot::open_station`], then the look at the screen before the first click.
    async fn open_station_and_look(&mut self, pos: BlockPos, kinds: &[WindowType]) -> Result<(), ActionError> {
        self.open_station(pos, kinds).await?;
        self.human_pause(human::SCREEN_OPEN_LOOK).await
    }

    /// Closes the station after a human pause.
    async fn close_station(&mut self) -> Result<(), ActionError> {
        self.human_pause(human::SCREEN_LINGER).await?;
        self.close_container().await
    }

    async fn click_pause(&mut self) -> Result<(), ActionError> {
        self.human_pause(human::CLICK).await
    }

    async fn human_pause(&mut self, range: human::Range) -> Result<(), ActionError> {
        let delay = self.human.between(range);
        self.pause(delay).await
    }

    /// One click moving `count` items from `from` into `to`.
    async fn put(&mut self, from: SlotRef, to: SlotRef, count: u8) -> Result<(), ActionError> {
        self.click_pause().await?;
        self.move_item(from, to, count).await
    }

    /// Moves `count` items accepted by `wanted` (identifier, metadata) from the main inventory
    /// into `to`, one click per source stack.
    async fn put_matching(&mut self, wanted: impl Fn(&str, u32) -> bool, count: u16, to: SlotRef) -> Result<(), ActionError> {
        let sources = matching_slots(&self.state, &wanted);
        if sources.iter().map(|(_, n)| *n).sum::<u16>() < count {
            return Err(ActionError::NotPossible(format!("not enough items for {to:?}: need {count}")));
        }
        let mut left = count;
        for (slot, available) in sources {
            if left == 0 {
                break;
            }
            let n = left.min(available);
            self.put(slot, to, n as u8).await?;
            left -= n;
        }
        Ok(())
    }

    /// One click sending a craft.
    async fn craft_click(&mut self, (craft, ops): (Craft, Vec<Op>)) -> Result<(), ActionError> {
        self.human_pause(human::CHOOSE).await?;
        self.craft_request(&craft, &ops).await
    }

    /// Shift-clicks whatever is left in `slots` back into the inventory.
    async fn take_back(&mut self, slots: &[SlotRef]) -> Result<(), ActionError> {
        for &slot in slots {
            if slot.stack(&self.state).is_some_and(|s| !s.is_empty()) {
                self.click_pause().await?;
                self.quick_move(slot).await?;
            }
        }
        Ok(())
    }
}

/// Main-inventory slots whose item `wanted` accepts, with their counts.
pub(crate) fn matching_slots(state: &GameState, wanted: impl Fn(&str, u32) -> bool) -> Vec<(SlotRef, u16)> {
    (0..Inventory::MAIN_SLOTS as u8)
        .map(SlotRef::Main)
        .filter_map(|slot| {
            let stack = slot.stack(state).filter(|s| !s.is_empty())?;
            let name = state.items.name(stack.network_id)?;
            wanted(name, stack.metadata).then_some((slot, stack.count))
        })
        .collect()
}

/// `stack` with its identifier, as craft results carry it.
pub(crate) fn named(state: &GameState, stack: &ItemStack) -> Result<(String, ItemStack), ActionError> {
    let name = state.items.name(stack.network_id).ok_or_else(|| ActionError::NotPossible(format!("item {} is not in the registry", stack.network_id)))?;
    Ok((name.to_owned(), stack.clone()))
}

/// The occupied stack in `slot`.
pub(crate) fn occupied(state: &GameState, slot: SlotRef) -> Result<&ItemStack, ActionError> {
    slot.stack(state).filter(|s| !s.is_empty()).ok_or_else(|| ActionError::NotPossible(format!("{slot:?} is empty")))
}
