//! Crafting by hand: each recipe cell's ingredients are laid out in the grid (UI slots 28-31 or
//! 32-40), then a shift-click on the result sends `CraftRecipe` for all crafts at once, `Consume`s
//! every occupied cell and places the results into the inventory. Each batch empties the grid;
//! leftovers after an error are taken back.
//!
//! Laying out follows vanilla (2026-10-02 capture): a whole stack for one cell is one click from
//! the inventory; otherwise the stack is picked up into the cursor, placed cell by cell, and what is
//! left goes back.

use std::collections::HashMap;

use super::craft::{pick_recipe, result_ops, results};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{ui, Op, SlotRef};
use crate::state::{GameState, Ingredient, Recipe, RecipeKind};
use crate::{ActionError, Bot};

/// Assumed maximum stack size of grid cells and results.
const MAX_STACK: u16 = 64;

/// `count` items from a main-inventory slot into a grid cell.
pub(crate) type Placement = (SlotRef, SlotRef, u8);

impl Bot {
    pub(super) async fn grid_craft_loop(&mut self, name: &str, count: u32, table: bool) -> Result<u32, ActionError> {
        let mut made = 0;
        while made < count {
            let batch = pick_recipe(&self.state, name, table).and_then(|r| Ok((r.clone(), grid_batch(&self.state, r, table, count - made)?)));
            let (recipe, (times, placements)) = match batch {
                Ok(batch) => batch,
                Err(ActionError::NotPossible(_)) if made > 0 => break,
                Err(e) => return Err(e),
            };
            for op in placement_clicks(&self.state, &placements) {
                self.click_pause().await?;
                self.item_stack_request(&[op]).await?;
            }
            let plan = grid_craft_plan(&self.state, &recipe, table, times)?;
            let output = plan.0.created().first().map_or(0, |s| u32::from(s.count));
            self.craft_click(plan).await?;
            made += output;
        }
        Ok(made)
    }
}

/// The grid cell each non-empty ingredient of `recipe` goes into, row by row from the top left.
pub(crate) fn grid_layout(recipe: &Recipe, table: bool) -> Result<Vec<(SlotRef, &Ingredient)>, ActionError> {
    let (first, side) = if table { (ui::CRAFTING_3X3, 3) } else { (ui::CRAFTING_2X2, 2) };
    let too_big = || ActionError::NotPossible(format!("recipe {} does not fit a {side}x{side} grid", recipe.id));
    let cells: Vec<(u8, &Ingredient)> = match recipe.kind {
        RecipeKind::Shaped { width, height } => {
            if width > side || height > side {
                return Err(too_big());
            }
            let cell = |i: usize| (i / usize::from(width)) as u8 * side + (i % usize::from(width)) as u8;
            recipe.inputs.iter().enumerate().map(|(i, ingredient)| (cell(i), ingredient)).collect()
        }
        RecipeKind::Shapeless => recipe.inputs.iter().filter(|i| !i.is_empty()).enumerate().map(|(i, ingredient)| (i as u8, ingredient)).collect(),
        _ => return Err(ActionError::NotPossible(format!("recipe {} is not a grid recipe", recipe.id))),
    };
    if cells.iter().any(|(cell, _)| *cell >= side * side) {
        return Err(too_big());
    }
    Ok(cells.into_iter().filter(|(_, i)| !i.is_empty()).map(|(cell, i)| (SlotRef::Ui(first + cell), i)).collect())
}

/// How many crafts the next batch makes (toward `wanted` items, within what the inventory holds
/// and a stack per cell and result holds) and what goes into each cell.
pub(crate) fn grid_batch(state: &GameState, recipe: &Recipe, table: bool, wanted: u32) -> Result<(u8, Vec<Placement>), ActionError> {
    let layout = grid_layout(recipe, table)?;
    if !state.inventory.cursor().is_empty() || layout.iter().any(|(cell, _)| cell.stack(state).is_some_and(|s| !s.is_empty())) {
        return Err(ActionError::NotPossible("the cursor and crafting grid must be empty".into()));
    }
    let per_craft = recipe.outputs.first().map_or(1, |o| u32::from(o.count.max(1)));
    let fit = |n: u16| u32::from(MAX_STACK / n.max(1));
    let cap = recipe.outputs.iter().map(|o| fit(o.count)).chain(layout.iter().map(|(_, i)| fit(i.count()))).min().unwrap_or(1);
    let most = wanted.div_ceil(per_craft).min(cap).clamp(1, u32::from(u8::MAX)) as u8;
    (1..=most)
        .rev()
        .find_map(|times| placements(state, &layout, times).ok().map(|p| (times, p)))
        .ok_or_else(|| ActionError::NotPossible(format!("missing ingredients for {}", recipe.id)))
}

/// What fills every cell of `layout` for `times` crafts from the main inventory, hotbar first.
fn placements(state: &GameState, layout: &[(SlotRef, &Ingredient)], times: u8) -> Result<Vec<Placement>, ActionError> {
    let mut left: HashMap<u8, u16> = HashMap::new();
    let mut moves = Vec::new();
    for (cell, ingredient) in layout {
        let mut need = ingredient.count() * u16::from(times);
        for (slot, stack) in state.inventory.main.iter().enumerate() {
            let slot = slot as u8;
            let Some(name) = state.items.name(stack.network_id).filter(|_| !stack.is_empty()) else { continue };
            let available = *left.entry(slot).or_insert(stack.count);
            if need == 0 || available == 0 || !ingredient.accepts(name, stack.metadata) {
                continue;
            }
            let n = need.min(available);
            moves.push((SlotRef::Main(slot), *cell, n as u8));
            left.insert(slot, available - n);
            need -= n;
        }
        if need > 0 {
            return Err(ActionError::NotPossible(format!("missing ingredient {ingredient:?}")));
        }
    }
    Ok(moves)
}

/// The clicks (one request each) carrying out `placements`, per source stack in order of first use.
pub(crate) fn placement_clicks(state: &GameState, placements: &[Placement]) -> Vec<Op> {
    let mut sources: Vec<(SlotRef, Vec<(SlotRef, u8)>)> = Vec::new();
    for &(from, cell, n) in placements {
        match sources.iter_mut().find(|(s, _)| *s == from) {
            Some((_, cells)) => cells.push((cell, n)),
            None => sources.push((from, vec![(cell, n)])),
        }
    }
    let mut clicks = Vec::new();
    for (from, cells) in sources {
        let held = from.stack(state).map_or(0, |s| u8::try_from(s.count).unwrap_or(u8::MAX));
        let used: u8 = cells.iter().map(|(_, n)| n).sum();
        if let [(cell, n)] = cells[..]
            && n == held
        {
            clicks.push(Op::Transfer { from, to: cell, count: n });
            continue;
        }
        clicks.push(Op::Transfer { from, to: SlotRef::Cursor, count: held });
        clicks.extend(cells.into_iter().map(|(cell, n)| Op::Transfer { from: SlotRef::Cursor, to: cell, count: n }));
        if held > used {
            clicks.push(Op::Transfer { from: SlotRef::Cursor, to: from, count: held - used });
        }
    }
    clicks
}

/// The shift-click on the result once the grid holds `times` crafts of `recipe`.
pub(crate) fn grid_craft_plan(state: &GameState, recipe: &Recipe, table: bool, times: u8) -> Result<(Craft, Vec<Op>), ActionError> {
    let mut ops = Vec::new();
    for (cell, ingredient) in grid_layout(recipe, table)? {
        let count = ingredient.count() * u16::from(times);
        if cell.stack(state).is_none_or(|s| s.count < count) {
            return Err(ActionError::NotPossible(format!("{cell:?} holds too few items for {times} crafts")));
        }
        ops.push(Op::Consume { from: cell, count: count as u8 });
    }
    let craft = Craft::new(CraftAction::Recipe { network_id: recipe.network_id, times }, results(state, recipe)?).with_results_action();
    ops.extend(result_ops(state, &craft, &ops)?);
    Ok((craft, ops))
}
