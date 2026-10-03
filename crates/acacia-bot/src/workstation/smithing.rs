//! Stonecutter and smithing table: `CraftRecipe` with a `CraftingData` recipe whose block is
//! `stonecutter` / `smithing_table`, consuming the inputs placed in their UI slots.

use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use super::{matching_slots, named, occupied};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack, Recipe, RecipeKind};
use crate::{ActionError, Bot};

/// Assumed maximum stack size of a stonecutter result.
const MAX_STACK: u16 = 64;

impl Bot {
    /// Cuts at least `count` of item `name` at the stonecutter at `pos`. Returns how many were made
    /// (fewer only if the input ran out after the first cut).
    pub async fn stonecut(&mut self, pos: BlockPos, name: &str, count: u32) -> Result<u32, ActionError> {
        let recipe = stonecutter_recipe(&self.state, name)?;
        let network_id = recipe.network_id;
        let (from, available) = matching_slots(&self.state, |n, m| recipe.inputs[0].accepts(n, m))[0];
        self.open_station_and_look(pos, &[WindowType::Stonecutter]).await?;
        let result = self.stonecut_open(network_id, from, available as u8, count).await;
        let back = self.take_back(&[SlotRef::Ui(ui::STONECUTTER_INPUT)]).await;
        self.close_station().await?;
        let made = result?;
        back.map(|()| made)
    }

    async fn stonecut_open(&mut self, network_id: u32, from: SlotRef, available: u8, count: u32) -> Result<u32, ActionError> {
        let input = SlotRef::Ui(ui::STONECUTTER_INPUT);
        self.put(from, input, available).await?;
        let mut made = 0;
        while made < count && occupied(&self.state, input).is_ok() {
            let recipe = self.state.recipes.book().get(network_id).ok_or_else(|| ActionError::NotPossible("the recipe is gone".into()))?;
            let plan = stonecut_plan(&self.state, recipe, count - made)?;
            made += u32::from(plan.0.created()[0].count);
            self.craft_click(plan).await?;
        }
        Ok(made)
    }

    /// Upgrades or trims `base` at the smithing table at `pos` with `addition` and `template`.
    pub async fn smith(&mut self, pos: BlockPos, base: SlotRef, addition: SlotRef, template: SlotRef) -> Result<(), ActionError> {
        // Vanilla filled template, material, then base (2026-10-02 capture).
        let slots = [SlotRef::Ui(ui::SMITHING_TEMPLATE), SlotRef::Ui(ui::SMITHING_MATERIAL), SlotRef::Ui(ui::SMITHING_INPUT)];
        self.workstation_craft(pos, WindowType::SmithingTable, &[Some(template), Some(addition), Some(base)], &slots, smithing_plan).await
    }
}

/// A stonecutter recipe for `name` whose input the inventory holds.
fn stonecutter_recipe<'a>(state: &'a GameState, name: &str) -> Result<&'a Recipe, ActionError> {
    state
        .recipes_for(name)
        .into_iter()
        .filter(|r| r.block == "stonecutter" && r.inputs.len() == 1)
        .find(|r| !matching_slots(state, |n, m| r.inputs[0].accepts(n, m)).is_empty())
        .ok_or_else(|| ActionError::NotPossible(format!("no stonecutter recipe for {name} with the items at hand")))
}

/// As many cuts as `wanted` items need, the input allows and one inventory stack holds.
pub(crate) fn stonecut_plan(state: &GameState, recipe: &Recipe, wanted: u32) -> Result<(Craft, Vec<Op>), ActionError> {
    let input = SlotRef::Ui(ui::STONECUTTER_INPUT);
    let (stack, per_cut) = (occupied(state, input)?, recipe.inputs[0].count().max(1));
    let output = recipe.outputs.first().ok_or_else(|| ActionError::NotPossible("recipe without result".into()))?;
    let wanted_cuts = wanted.div_ceil(u32::from(output.count.max(1)));
    let cuts = wanted_cuts.min(u32::from(stack.count / per_cut)).min(u32::from(MAX_STACK / output.count.max(1)));
    if cuts == 0 {
        return Err(ActionError::NotPossible("not enough input for a cut".into()));
    }
    let craft = Craft::new(CraftAction::Recipe { network_id: recipe.network_id, times: cuts as u8 }, vec![named(state, output)?]);
    let mut ops = vec![Op::Consume { from: input, count: (cuts as u16 * per_cut) as u8 }];
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &craft.created()[0])?);
    Ok((craft, ops))
}

pub(crate) fn smithing_plan(state: &GameState) -> Result<(Craft, Vec<Op>), ActionError> {
    let slots = [ui::SMITHING_TEMPLATE, ui::SMITHING_INPUT, ui::SMITHING_MATERIAL].map(SlotRef::Ui);
    let stacks = slots.map(|slot| occupied(state, slot));
    let [Ok(template), Ok(base), Ok(addition)] = stacks else {
        return Err(ActionError::NotPossible("the smithing table needs a template, an item and a material".into()));
    };
    let fits = |r: &&Recipe| {
        [template, base, addition].iter().zip(&r.inputs).all(|(s, i)| state.items.name(s.network_id).is_some_and(|n| i.accepts(n, s.metadata)))
    };
    let recipe = state
        .recipes
        .book()
        .iter()
        .filter(|r| r.block == "smithing_table" && matches!(r.kind, RecipeKind::SmithingTransform | RecipeKind::SmithingTrim))
        .find(fits)
        .ok_or_else(|| ActionError::NotPossible("no smithing recipe for these items".into()))?;
    // A transform keeps the base's enchantments and name; a trim changes only the base's NBT.
    let result = match recipe.outputs.first() {
        Some(out) => ItemStack { count: 1, nbt: base.nbt.clone(), custom_name: base.custom_name.clone(), ..out.clone() },
        None => ItemStack { count: 1, ..base.clone() },
    };
    let craft = Craft::new(CraftAction::Recipe { network_id: recipe.network_id, times: 1 }, vec![named(state, &result)?]).with_results_action();
    let mut ops: Vec<Op> = [ui::SMITHING_TEMPLATE, ui::SMITHING_INPUT, ui::SMITHING_MATERIAL]
        .map(|slot| Op::Consume { from: SlotRef::Ui(slot), count: 1 })
        .to_vec();
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?);
    Ok((craft, ops))
}
