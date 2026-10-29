//! A banner onto a shield in the own 2×2 grid: a built-in recipe. Its UUID is the one BDS 1.26
//! announces right after the two banner recipes and no public list names; that it is the
//! shield's is by exclusion and by the craft going through (2026-10-10).

use acacia_client::proto::nbt::Value;

use super::anvil::set_nbt;
use super::{named, occupied};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

const RECIPE: &str = "00000000-0000-0000-0000-0000000000c8";

impl Bot {
    /// Crafts the banner in `banner` onto the shield in `shield`; the shield lands back in the
    /// inventory with the banner's dye and patterns.
    pub async fn decorate_shield(&mut self, shield: SlotRef, banner: SlotRef) -> Result<(), ActionError> {
        let cells = [SlotRef::Ui(ui::CRAFTING_2X2), SlotRef::Ui(ui::CRAFTING_2X2 + 1)];
        let screen = self.hold_own_inventory_screen().await?;
        self.move_item(shield, cells[0], 1).await?;
        self.move_item(banner, cells[1], 1).await?;
        let crafted = match plan(&self.state, cells) {
            Ok(plan) => self.craft_click(plan).await,
            Err(e) => Err(e),
        };
        let back = self.take_back(&cells).await;
        if let Some(screen) = screen {
            self.release(screen);
        }
        crafted.and(back)
    }
}

/// The craft for a shield and a banner lying in `cells`: the shield takes the banner's dye (its
/// aux) as `Base` and its `Patterns`.
fn plan(state: &GameState, [shield, banner]: [SlotRef; 2]) -> Result<(Craft, Vec<Op>), ActionError> {
    let (plain, cloth) = (occupied(state, shield)?, occupied(state, banner)?);
    let recipe = state.recipes.book().multi_recipe(RECIPE).ok_or_else(|| ActionError::NotPossible("the server sent no shield decoration recipe".into()))?;
    let mut result = ItemStack { count: 1, ..plain.clone() };
    set_nbt(&mut result, "Base", Value::Int(cloth.metadata as i32));
    if let Some(patterns) = cloth.nbt.as_ref().and_then(|nbt| nbt.value.get("Patterns")) {
        set_nbt(&mut result, "Patterns", patterns.clone());
    }
    let craft = Craft::new(CraftAction::Recipe { network_id: recipe, times: 1 }, vec![named(state, &result)?]).with_results_action();
    let mut ops = vec![Op::Consume { from: shield, count: 1 }, Op::Consume { from: banner, count: 1 }];
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?);
    Ok((craft, ops))
}
