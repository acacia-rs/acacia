//! A craft from the grid as a player filled it by hand (the viewer's screens): which recipe the
//! cells make and the click on the result. A shaped recipe may sit anywhere in the grid, also
//! mirrored.

use acacia_client::proto::types::WindowType;

use super::anvil::anvil_plan;
use super::cartography::cartography_plan;
use super::craft::{grid_cells, result_ops, results};
use super::grindstone::grindstone_plan;
use super::smithing::smithing_plan;
use crate::items::craft::{Craft, CraftAction};
use crate::items::{is_own_screen, onto_cursor, ui, Op, SlotRef};
use crate::state::{GameState, Ingredient, ItemStack, Recipe, RecipeKind};
use crate::{ActionError, Bot};

/// Assumed maximum stack size of a craft result.
const MAX_STACK: u16 = 64;

/// A grid cell's item: identifier, metadata, count.
type Cell<'a> = Option<(&'a str, u32, u16)>;

impl Bot {
    /// What the open screen's workstation slots make now: the own 2x2 grid, a crafting table's,
    /// an anvil's or cartography table's (with the new name `name`, if any), a smithing table's or
    /// a grindstone's.
    pub fn station_result(&self, name: Option<&str>) -> Option<ItemStack> {
        station_plan(&self.state, false, name).ok()?.0.created().into_iter().next()
    }

    /// The click on that result: one craft onto the cursor, or with `all` (a shift-click) as many
    /// as a crafting grid's cells hold into the inventory.
    pub async fn take_station_result(&mut self, all: bool, name: Option<&str>) -> Result<(), ActionError> {
        let (craft, ops) = station_plan(&self.state, all, name)?;
        let ops = if all { ops } else { onto_cursor(&self.state, &craft.created(), ops) };
        self.craft_request(&craft, &ops).await
    }

    /// Shift-clicks what is left in the workstation slots back into the inventory, as before a
    /// screen closes.
    pub async fn take_back_ui(&mut self) -> Result<(), ActionError> {
        for slot in (1..ui::CREATED_OUTPUT).chain(ui::CREATED_OUTPUT + 1..=ui::SMITHING_TEMPLATE).map(SlotRef::Ui) {
            if slot.stack(&self.state).is_some_and(|s| !s.is_empty()) {
                self.quick_move(slot).await?;
            }
        }
        Ok(())
    }
}

fn station_plan(state: &GameState, all: bool, name: Option<&str>) -> Result<(Craft, Vec<Op>), ActionError> {
    match state.containers.open.as_ref().filter(|c| !is_own_screen(c)).map(|c| c.window_type) {
        None => hand_craft_plan(state, false, all),
        Some(WindowType::Workbench) => hand_craft_plan(state, true, all),
        Some(WindowType::Anvil) => anvil_plan(state, name),
        Some(WindowType::SmithingTable) => smithing_plan(state),
        Some(WindowType::Grindstone) => grindstone_plan(state),
        Some(WindowType::Cartography) => cartography_plan(state, name),
        Some(other) => Err(ActionError::NotPossible(format!("a {other:?} window makes nothing by hand"))),
    }
}

/// The first crafting recipe the grid's contents make, and what one craft uses up of each cell.
pub(crate) fn grid_match(state: &GameState, table: bool) -> Option<(&Recipe, Vec<(SlotRef, u16)>)> {
    let slots = grid_cells(table);
    let side = if table { 3 } else { 2 };
    let cells: Vec<Cell> = slots
        .iter()
        .map(|slot| {
            let stack = slot.stack(state).filter(|s| !s.is_empty())?;
            Some((state.items.name(stack.network_id).unwrap_or_default(), stack.metadata, stack.count))
        })
        .collect();
    if cells.iter().all(Option::is_none) {
        return None;
    }
    let book = state.recipes.book();
    book.iter().filter(|r| r.block == "crafting_table").find_map(|recipe| {
        let used = uses(recipe, &cells, side)?;
        Some((recipe, used.into_iter().map(|(cell, n)| (slots[cell], n)).collect()))
    })
}

/// What one craft of `recipe` takes from each cell (by index), if the cells are that recipe.
fn uses(recipe: &Recipe, cells: &[Cell], side: usize) -> Option<Vec<(usize, u16)>> {
    let filled: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].is_some()).collect();
    match recipe.kind {
        RecipeKind::Shaped { width, height } => {
            let (w, h) = (usize::from(width), usize::from(height));
            let (cols, rows) = (filled.iter().map(|i| i % side), filled.iter().map(|i| i / side));
            let (left, top) = (cols.clone().min()?, rows.clone().min()?);
            if cols.max()? - left + 1 != w || rows.max()? - top + 1 != h || recipe.inputs.len() != w * h {
                return None;
            }
            [false, true].into_iter().find_map(|mirrored| {
                let mut used = Vec::new();
                for (i, ingredient) in recipe.inputs.iter().enumerate() {
                    let col = if mirrored { w - 1 - i % w } else { i % w };
                    let cell = (top + i / w) * side + left + col;
                    match cells[cell] {
                        None if ingredient.is_empty() => {}
                        Some((name, metadata, count)) if !ingredient.is_empty() && ingredient.accepts(name, metadata) && count >= ingredient.count() => {
                            used.push((cell, ingredient.count()))
                        }
                        _ => return None,
                    }
                }
                Some(used)
            })
        }
        RecipeKind::Shapeless => {
            let wanted: Vec<&Ingredient> = recipe.inputs.iter().filter(|i| !i.is_empty()).collect();
            let (mut free, mut used) = (filled, Vec::new());
            (wanted.len() == free.len() && assign(&wanted, &mut free, cells, &mut used)).then_some(used)
        }
        _ => None,
    }
}

/// Gives each of `wanted` one of the `free` cells that holds it, trying every pairing.
fn assign(wanted: &[&Ingredient], free: &mut Vec<usize>, cells: &[Cell], used: &mut Vec<(usize, u16)>) -> bool {
    let Some((first, rest)) = wanted.split_first() else { return true };
    for k in 0..free.len() {
        let cell = free[k];
        let Some((name, metadata, count)) = cells[cell] else { continue };
        if !first.accepts(name, metadata) || count < first.count() {
            continue;
        }
        free.remove(k);
        used.push((cell, first.count()));
        if assign(rest, free, cells, used) {
            return true;
        }
        used.pop();
        free.insert(k, cell);
    }
    false
}

pub(crate) fn hand_craft_plan(state: &GameState, table: bool, all: bool) -> Result<(Craft, Vec<Op>), ActionError> {
    let (recipe, used) = grid_match(state, table).ok_or_else(|| ActionError::NotPossible("the grid makes nothing".into()))?;
    let held = |slot: SlotRef| slot.stack(state).map_or(0, |s| s.count);
    let most = used.iter().map(|&(slot, n)| held(slot) / n.max(1)).min().unwrap_or(1);
    let cap = recipe.outputs.iter().map(|o| MAX_STACK / o.count.max(1)).min().unwrap_or(1);
    let times = if all { most.min(cap).clamp(1, u16::from(u8::MAX)) } else { 1 };
    let mut ops: Vec<Op> = used.iter().map(|&(from, n)| Op::Consume { from, count: (n * times) as u8 }).collect();
    let craft = Craft::new(CraftAction::Recipe { network_id: recipe.network_id, times: times as u8 }, results(state, recipe)?).with_results_action();
    ops.extend(result_ops(state, &craft, &ops)?);
    Ok((craft, ops))
}
