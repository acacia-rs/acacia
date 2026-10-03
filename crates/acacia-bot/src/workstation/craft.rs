//! Crafting in the own 2x2 grid or at a crafting table, the recipe-book way (`CraftRecipeAuto`:
//! ingredients are consumed straight from the inventory, [`book_crafts`] per click) or by hand
//! ([`super::grid`]). Results go into the inventory; a recipe with several results gets a `Create`
//! per result (Geyser rejects those requests).

use std::collections::HashMap;

use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use super::named;
use crate::human;
use crate::items::craft::{Craft, CraftAction};
use crate::items::{is_own_screen, to_inventory_ops_after, ui, Op, SlotRef};
use crate::state::{Container, GameState, Ingredient, Inventory, ItemStack, Recipe, RecipeKind};
use crate::{ActionError, Bot};

/// Assumed maximum stack size of a craft result.
const MAX_STACK: u16 = 64;

/// How [`Bot::craft_with`] crafts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CraftMode {
    /// Click the recipe in the recipe book: one click crafting all if that is not more than
    /// wanted, else a click per craft.
    #[default]
    RecipeBook,
    /// Lay the ingredients out in the grid through the cursor, then shift-click the result (all
    /// crafts at once).
    Grid,
}

impl Bot {
    /// Recipes that make item `name` (`minecraft:stick`). The first lookup decodes `CraftingData`.
    pub fn recipes_for(&self, name: &str) -> Vec<&Recipe> {
        self.state.recipes_for(name)
    }

    /// Crafts at least `count` of item `name` through the recipe book: in the own 2x2 grid, or at
    /// the crafting table at `table`. Returns how many items were made, which falls short of
    /// `count` only when the ingredients ran out after at least one craft.
    pub async fn craft(&mut self, name: &str, count: u32, table: Option<BlockPos>) -> Result<u32, ActionError> {
        self.craft_with(name, count, table, CraftMode::RecipeBook).await
    }

    /// [`Bot::craft`] choosing between the recipe book and laying the ingredients out by hand.
    pub async fn craft_with(&mut self, name: &str, count: u32, table: Option<BlockPos>, mode: CraftMode) -> Result<u32, ActionError> {
        pick_recipe(&self.state, name, table.is_some())?;
        if table.is_none() && self.state.containers.open.as_ref().is_some_and(|c| !is_own_screen(c)) {
            return Err(ActionError::NotPossible("the 2x2 grid needs the open container closed first".into()));
        }
        let own_screen = match table {
            Some(pos) => {
                self.open_station_and_look(pos, &[WindowType::Workbench]).await?;
                false
            }
            None => {
                let opened = self.hold_own_inventory_screen().await?;
                if opened {
                    self.human_pause(human::SCREEN_OPEN_LOOK).await?;
                }
                opened
            }
        };
        let result = match mode {
            CraftMode::RecipeBook => self.craft_loop(name, count, table.is_some()).await,
            CraftMode::Grid => self.grid_craft_loop(name, count, table.is_some()).await,
        };
        let back = self.take_back(&grid_cells(table.is_some())).await;
        if table.is_some() {
            self.close_station().await?;
        } else if own_screen {
            self.human_pause(human::SCREEN_LINGER).await?;
            self.close_own_inventory_screen();
        }
        let made = result?;
        back.map(|()| made)
    }

    async fn craft_loop(&mut self, name: &str, count: u32, table: bool) -> Result<u32, ActionError> {
        let mut made = 0;
        while made < count {
            let plan = pick_recipe(&self.state, name, table)
                .and_then(|recipe| auto_craft_plan(&self.state, recipe, book_crafts(&self.state, recipe, count - made)));
            let plan = match plan {
                Ok(plan) => plan,
                Err(ActionError::NotPossible(_)) if made > 0 => break,
                Err(e) => return Err(e),
            };
            let output = plan.0.created().first().map_or(0, |s| u32::from(s.count));
            self.craft_click(plan).await?;
            made += output;
        }
        Ok(made)
    }

    /// Opens the own inventory screen for several requests; true if it was opened here. BDS may
    /// not confirm the open, so the screen is then recorded as open locally.
    async fn hold_own_inventory_screen(&mut self) -> Result<bool, ActionError> {
        let opened = self.open_own_inventory_screen().await?;
        if opened && self.state.containers.open.is_none() {
            self.state.containers.open = Some(Container {
                window_id: Inventory::WINDOW_INVENTORY,
                window_type: WindowType::Inventory,
                position: None,
                entity: None,
                slots: Vec::new(),
            });
        }
        Ok(opened)
    }
}

/// The UI slots of the 3x3 table grid or the own 2x2 grid.
pub(crate) fn grid_cells(table: bool) -> Vec<SlotRef> {
    let (first, len) = if table { (ui::CRAFTING_3X3, 9) } else { (ui::CRAFTING_2X2, 4) };
    (first..first + len).map(SlotRef::Ui).collect()
}

/// The first grid recipe for `name` that fits the grid and whose ingredients the inventory holds.
pub(crate) fn pick_recipe<'a>(state: &'a GameState, name: &str, table: bool) -> Result<&'a Recipe, ActionError> {
    let candidates: Vec<&Recipe> = state
        .recipes_for(name)
        .into_iter()
        .filter(|r| r.block == "crafting_table" && matches!(r.kind, RecipeKind::Shaped { .. } | RecipeKind::Shapeless))
        .filter(|r| table || r.fits_2x2())
        .collect();
    if candidates.is_empty() {
        let grid = if table { "a crafting table" } else { "the 2x2 grid" };
        return Err(ActionError::NotPossible(format!("no recipe for {name} in {grid}")));
    }
    candidates
        .iter()
        .find(|r| consumes(state, r, 1).is_ok())
        .copied()
        .ok_or_else(|| ActionError::NotPossible(format!("missing ingredients for {name}")))
}

/// How many crafts one recipe-book click makes toward `wanted` items: everything the inventory
/// allows (a shift-click, as vanilla crafted two fences at once) when that is not more than
/// wanted, else one per click.
pub(crate) fn book_crafts(state: &GameState, recipe: &Recipe, wanted: u32) -> u8 {
    let per_craft = recipe.outputs.first().map_or(1, |o| o.count.max(1));
    let cap = (MAX_STACK / per_craft).clamp(1, u16::from(u8::MAX)) as u8;
    let most = (1..=cap).rev().find(|&times| consumes(state, recipe, times).is_ok()).unwrap_or(1);
    if wanted.div_ceil(u32::from(per_craft)) >= u32::from(most) { most } else { 1 }
}

/// A recipe-book click crafting `recipe` `times` times: `Consume`s grouped per recipe cell in
/// recipe order (Geyser requires that grouping), then the results placed into the inventory.
pub(crate) fn auto_craft_plan(state: &GameState, recipe: &Recipe, times: u8) -> Result<(Craft, Vec<Op>), ActionError> {
    let (cursor, grid) = (state.inventory.cursor(), ui::CRAFTING_2X2..=ui::CRAFTING_3X3_LAST);
    if !cursor.is_empty() || grid.clone().any(|i| !state.inventory.ui[usize::from(i)].is_empty()) {
        return Err(ActionError::NotPossible("the cursor and crafting grid must be empty".into()));
    }
    let (mut ops, ingredients) = consumes(state, recipe, times)?;
    let craft = Craft::new(CraftAction::Auto { network_id: recipe.network_id, times, ingredients }, results(state, recipe)?).with_results_action();
    ops.extend(result_ops(state, &craft, &ops)?);
    Ok((craft, ops))
}

/// `recipe`'s results with their identifiers.
pub(crate) fn results(state: &GameState, recipe: &Recipe) -> Result<Vec<(String, ItemStack)>, ActionError> {
    if recipe.outputs.is_empty() {
        return Err(ActionError::NotPossible(format!("recipe {} has no result", recipe.id)));
    }
    recipe.outputs.iter().map(|output| named(state, output)).collect()
}

/// Ops moving `craft`'s results from the created-output slot into the inventory as the `earlier`
/// ops of the request leave it. Several results take turns there: vanilla sends `Create` for
/// each, then moves it out (Geyser #3682 dump).
pub(crate) fn result_ops(state: &GameState, craft: &Craft, earlier: &[Op]) -> Result<Vec<Op>, ActionError> {
    let created = craft.created();
    if let [single] = created.as_slice() {
        return to_inventory_ops_after(state, SlotRef::CREATED_OUTPUT, single, earlier);
    }
    let (mut ops, mut done) = (Vec::new(), earlier.to_vec());
    for (index, stack) in created.iter().enumerate() {
        ops.push(Op::Create { index: index as u8 });
        let moves = to_inventory_ops_after(state, SlotRef::CREATED_OUTPUT, stack, &done)?;
        done.extend(moves.iter().copied());
        ops.extend(moves);
    }
    Ok(ops)
}

/// `Consume`s from the main inventory for every non-empty cell of `recipe`, `times` crafts each,
/// and the cells as the request describes them (per-craft counts): BDS cannot match an item-tag or
/// MoLang descriptor against the consumed items (status 25; a tag counting 2 crashed it), so those
/// cells name the item they use up.
fn consumes(state: &GameState, recipe: &Recipe, times: u8) -> Result<(Vec<Op>, Vec<Ingredient>), ActionError> {
    let mut left: HashMap<u8, u16> = HashMap::new();
    let (mut ops, mut cells) = (Vec::new(), Vec::new());
    for cell in recipe.inputs.iter().filter(|i| !i.is_empty()) {
        let mut need = cell.count() * u16::from(times);
        let mut used = None;
        for (slot, stack) in state.inventory.main.iter().enumerate() {
            let slot = slot as u8;
            let Some(name) = state.items.name(stack.network_id).filter(|_| !stack.is_empty()) else { continue };
            let available = *left.entry(slot).or_insert(stack.count);
            if need == 0 || available == 0 || !cell.accepts(name, stack.metadata) {
                continue;
            }
            let n = need.min(available);
            ops.push(Op::Consume { from: SlotRef::Main(slot), count: n as u8 });
            left.insert(slot, available - n);
            need -= n;
            used.get_or_insert((name, stack.metadata));
        }
        if need > 0 {
            return Err(ActionError::NotPossible(format!("missing ingredient {cell:?} for {}", recipe.id)));
        }
        cells.push(match (cell, used) {
            (Ingredient::Tag { count, .. } | Ingredient::Molang { count, .. }, Some((name, metadata))) => {
                Ingredient::Item { name: name.to_owned(), metadata: Some(metadata as i32), count: *count }
            }
            _ => cell.clone(),
        });
    }
    Ok((ops, cells))
}
