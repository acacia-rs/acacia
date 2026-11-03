//! Moving items by name between the open container (chest, barrel, shulker box, ...) and the main inventory.

use super::matching_slots;
use crate::items::{open_container, quick_move_up_to_ops, Op, SlotRef};
use crate::state::GameState;
use crate::{ActionError, Bot};

impl Bot {
    /// Moves up to `count` items named `name` (`minecraft:cobblestone`) from the main inventory into the open
    /// container, a shift-click per source stack (only the rest of the last one). Returns how many moved: fewer than
    /// `count` once the inventory has no more or the container is full. `NotPossible` with no container open, none
    /// of the item in the inventory, or no room for any.
    pub async fn deposit(&mut self, name: &str, count: u32) -> Result<u32, ActionError> {
        let sources: Vec<SlotRef> = matching_slots(&self.state, |item, _| item == name).into_iter().map(|(slot, _)| slot).collect();
        self.move_named(name, count, sources).await
    }

    /// [`Bot::deposit`] the other way: from the open container into the main inventory.
    pub async fn withdraw(&mut self, name: &str, count: u32) -> Result<u32, ActionError> {
        let sources = container_slots(&self.state, name);
        self.move_named(name, count, sources).await
    }

    async fn move_named(&mut self, name: &str, count: u32, sources: Vec<SlotRef>) -> Result<u32, ActionError> {
        if open_container(&self.state).is_none() {
            return Err(ActionError::NotPossible("no container is open".into()));
        }
        if sources.is_empty() {
            return Err(ActionError::NotPossible(format!("no {name} to move")));
        }
        let mut moved = 0;
        for from in sources {
            if moved >= count {
                break;
            }
            let left = u16::try_from(count - moved).unwrap_or(u16::MAX);
            let ops = match quick_move_up_to_ops(&self.state, from, left) {
                Ok(ops) => ops,
                Err(ActionError::NotPossible(_)) if moved > 0 => break,
                Err(e) => return Err(e),
            };
            self.click_pause().await?;
            self.item_stack_request(&ops).await?;
            moved += ops.iter().map(|op| if let Op::Transfer { count, .. } = op { u32::from(*count) } else { 0 }).sum::<u32>();
        }
        Ok(moved)
    }
}

/// The open container's slots holding item `name`.
fn container_slots(state: &GameState, name: &str) -> Vec<SlotRef> {
    let Some(open) = open_container(state) else { return Vec::new() };
    let named = |id| state.items.name(id) == Some(name);
    open.iter().filter(|(_, s)| named(s.network_id)).filter_map(|(i, _)| u8::try_from(i).ok().map(SlotRef::Container)).collect()
}
