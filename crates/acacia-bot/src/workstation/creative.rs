//! Taking items out of the creative inventory: `CraftCreative` with the entry's id, then the
//! created stack placed into the inventory.

use super::named;
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

impl Bot {
    /// Takes `count` of creative inventory entry `entry_id` into the inventory (creative mode).
    pub async fn take_creative(&mut self, entry_id: u32, count: u8) -> Result<(), ActionError> {
        let (craft, ops) = creative_plan(&self.state, entry_id, count)?;
        self.craft_request(&craft, &ops).await
    }
}

pub(crate) fn creative_plan(state: &GameState, entry_id: u32, count: u8) -> Result<(Craft, Vec<Op>), ActionError> {
    let items = state.creative.items();
    let item = items.iter().find(|i| i.entry_id == entry_id).ok_or_else(|| ActionError::NotPossible(format!("no creative item {entry_id}")))?;
    let result = ItemStack { count: u16::from(count.max(1)), ..item.stack.clone() };
    let craft = Craft::new(CraftAction::Creative { entry_id }, vec![named(state, &result)?]).with_results_action();
    let ops = to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?;
    Ok((craft, ops))
}
