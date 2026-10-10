//! Workstations where a player picks one of several results by hand (the viewer's screens): what
//! the stonecutter offers for the stack in its slot and the click on the result, and the
//! enchanting table's options, and a beacon's powers.

use super::beacon::beacon_plan;
use super::enchant::enchant_plan;
use super::BeaconEffect;
use super::occupied;
use super::smithing::stonecut_plan;
use crate::items::{onto_cursor, ui, SlotRef};
use crate::state::ItemStack;
use crate::{ActionError, Bot};

impl Bot {
    /// What the stonecutter makes of the stack in its slot: each recipe's network id and one
    /// cut's result, in the server's order.
    pub fn stonecutter_choices(&self) -> Vec<(u32, ItemStack)> {
        let state = &self.state;
        let Ok(input) = occupied(state, SlotRef::Ui(ui::STONECUTTER_INPUT)) else { return Vec::new() };
        let Some(name) = state.items.name(input.network_id) else { return Vec::new() };
        let cuts = state.recipes.book().iter().filter(|r| r.block == "stonecutter" && r.inputs.len() == 1 && r.inputs[0].accepts(name, input.metadata));
        cuts.filter_map(|r| Some((r.network_id, r.outputs.first()?.clone()))).collect()
    }

    /// The click on the stonecutter's result for recipe `network_id`: one cut onto the cursor, or
    /// with `all` as many as the input and a stack allow into the inventory.
    pub async fn take_stonecut(&mut self, network_id: u32, all: bool) -> Result<(), ActionError> {
        let recipe = self.state.recipes.book().get(network_id).ok_or_else(|| ActionError::NotPossible("the recipe is gone".into()))?;
        let (craft, ops) = stonecut_plan(&self.state, recipe, if all { u32::MAX } else { 1 })?;
        let ops = if all { ops } else { onto_cursor(&self.state, &craft.created(), ops) };
        self.craft_request(&craft, &ops).await
    }

    /// The level each of the enchanting table's options needs, cheapest first; none without an
    /// item in its slot.
    pub fn enchant_costs(&self) -> Vec<u8> {
        match occupied(&self.state, SlotRef::Ui(ui::ENCHANTING_INPUT)) {
            Ok(_) => self.state.stations.enchant_options.iter().map(|o| o.cost).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// The click on enchanting option `option`: the item in the table's slot is enchanted there.
    pub async fn take_enchant(&mut self, option: usize) -> Result<(), ActionError> {
        let (craft, ops) = enchant_plan(&self.state, option)?;
        let level = self.state.player.xp_level;
        self.craft_request(&craft, &ops).await?;
        self.pay_enchant_levels(level, option);
        Ok(())
    }

    /// The beacon screen's confirm button: sets the open beacon's powers, paid with the item in
    /// its slot.
    pub async fn take_beacon(&mut self, primary: BeaconEffect, secondary: Option<BeaconEffect>) -> Result<(), ActionError> {
        let (head, ops) = beacon_plan(&self.state, primary, secondary)?;
        self.headed_request(head, &ops).await
    }
}
