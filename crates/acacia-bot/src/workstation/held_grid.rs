//! Crafting what the grid already holds, as a click on the result slot does: the player laid the cells out
//! (a Java client through a bridge, a script mirroring a GUI). The cells are matched against the recipe book:
//! shaped recipes anywhere in the grid and mirrored, shapeless ones in any order.

use acacia_client::proto::types::WindowType;

use super::craft::{result_ops, results};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{onto_cursor, ui, Op, SlotRef};
use crate::state::{GameState, Ingredient, Recipe, RecipeKind};
use crate::{ActionError, Bot};

/// A recipe the grid holds: each filled cell with the items one craft uses from it.
pub(crate) struct Held<'a> {
    pub recipe: &'a Recipe,
    pub cells: Vec<(SlotRef, u16)>,
}

impl Bot {
    /// The recipe the own 2x2 grid, or the open crafting table's 3x3 grid, holds as laid out.
    pub fn grid_recipe(&self) -> Option<&Recipe> {
        held(&self.state).map(|h| h.recipe)
    }

    /// Crafts what the grid holds up to `times` times, as a click on the result slot: the results go onto the
    /// cursor with `to_cursor` when it is empty or holds the result with room (one craft is a left click), else
    /// into the inventory (a shift-click). Returns how many crafts were made.
    pub async fn craft_grid(&mut self, times: u8, to_cursor: bool) -> Result<u8, ActionError> {
        let (craft, ops) = held_grid_plan(&self.state, times, to_cursor)?;
        let made = craft.action.times();
        self.craft_request(&craft, &ops).await?;
        Ok(made)
    }
}

pub(crate) fn held_grid_plan(state: &GameState, times: u8, to_cursor: bool) -> Result<(Craft, Vec<Op>), ActionError> {
    let held = held(state).ok_or_else(|| ActionError::NotPossible("the grid holds no recipe".into()))?;
    let most = held.cells.iter().map(|(slot, per)| slot.stack(state).map_or(0, |s| s.count) / per).min().unwrap_or(0);
    let times = times.min(u8::try_from(most).unwrap_or(u8::MAX));
    if times == 0 {
        return Err(ActionError::NotPossible("nothing to craft".into()));
    }
    let mut ops: Vec<Op> = held.cells.iter().map(|&(from, per)| Op::Consume { from, count: (per * u16::from(times)) as u8 }).collect();
    let craft = Craft::new(CraftAction::Recipe { network_id: held.recipe.network_id, times }, results(state, held.recipe)?).with_results_action();
    ops.extend(result_ops(state, &craft, &ops)?);
    if to_cursor {
        ops = onto_cursor(state, &craft.created(), ops);
    }
    Ok((craft, ops))
}

/// The grid recipe the cells match; the lowest priority wins when several do.
pub(crate) fn held(state: &GameState) -> Option<Held<'_>> {
    let table = state.containers.open.as_ref().is_some_and(|c| c.window_type == WindowType::Workbench);
    let (first, side) = if table { (ui::CRAFTING_3X3, 3) } else { (ui::CRAFTING_2X2, 2) };
    let mut cells = Vec::new();
    for i in 0..side * side {
        let slot = SlotRef::Ui(first + i);
        cells.push(match slot.stack(state).filter(|s| !s.is_empty()) {
            Some(stack) => Some(Cell { slot, name: state.items.name(stack.network_id)?, metadata: stack.metadata, count: stack.count }),
            None => None,
        });
    }
    if cells.iter().all(Option::is_none) {
        return None;
    }
    let grid = Grid { cells, side };
    state
        .recipes
        .book()
        .iter()
        .filter(|r| r.block == "crafting_table")
        .filter_map(|recipe| Some(Held { recipe, cells: grid.matches(recipe)? }))
        .min_by_key(|h| h.recipe.priority)
}

struct Cell<'a> {
    slot: SlotRef,
    name: &'a str,
    metadata: u32,
    count: u16,
}

impl Cell<'_> {
    fn fills(&self, ingredient: &Ingredient) -> bool {
        ingredient.accepts(self.name, self.metadata) && self.count >= ingredient.count()
    }
}

struct Grid<'a> {
    cells: Vec<Option<Cell<'a>>>,
    side: u8,
}

impl Grid<'_> {
    fn matches(&self, recipe: &Recipe) -> Option<Vec<(SlotRef, u16)>> {
        match recipe.kind {
            RecipeKind::Shaped { width, height } => self.shaped(recipe, width, height),
            RecipeKind::Shapeless => self.shapeless(recipe),
            _ => None,
        }
    }

    fn shaped(&self, recipe: &Recipe, width: u8, height: u8) -> Option<Vec<(SlotRef, u16)>> {
        let filled = |i: usize| self.cells[i].is_some();
        let side = usize::from(self.side);
        let rows: Vec<usize> = (0..side).filter(|r| (0..side).any(|c| filled(r * side + c))).collect();
        let cols: Vec<usize> = (0..side).filter(|c| (0..side).any(|r| filled(r * side + c))).collect();
        let (top, left) = (*rows.first()?, *cols.first()?);
        let (w, h) = (usize::from(width), usize::from(height));
        // the filled box must be the recipe's: no stray cells outside it
        if rows.last()? - top + 1 > h || cols.last()? - left + 1 > w || top + h > side || left + w > side {
            return None;
        }
        [false, true].into_iter().find_map(|mirrored| {
            let mut used = Vec::new();
            for r in 0..h {
                for c in 0..w {
                    let ingredient = recipe.inputs.get(r * w + if mirrored { w - 1 - c } else { c }).unwrap_or(&Ingredient::Empty);
                    match (&self.cells[(top + r) * side + left + c], ingredient.is_empty()) {
                        (None, true) => {}
                        (Some(cell), false) if cell.fills(ingredient) => used.push((cell.slot, ingredient.count())),
                        _ => return None,
                    }
                }
            }
            Some(used)
        })
    }

    fn shapeless(&self, recipe: &Recipe) -> Option<Vec<(SlotRef, u16)>> {
        let ingredients: Vec<&Ingredient> = recipe.inputs.iter().filter(|i| !i.is_empty()).collect();
        let cells: Vec<&Cell> = self.cells.iter().flatten().collect();
        if ingredients.len() != cells.len() {
            return None;
        }
        let mut taken = vec![false; cells.len()];
        let mut used = Vec::new();
        assign(&ingredients, &cells, &mut taken, &mut used).then_some(used)
    }
}

/// Gives every ingredient its own cell, backtracking: a tag ingredient may take a cell an exact one needs.
fn assign(ingredients: &[&Ingredient], cells: &[&Cell], taken: &mut [bool], used: &mut Vec<(SlotRef, u16)>) -> bool {
    let Some((ingredient, rest)) = ingredients.split_first() else { return true };
    for (i, cell) in cells.iter().enumerate() {
        if taken[i] || !cell.fills(ingredient) {
            continue;
        }
        taken[i] = true;
        used.push((cell.slot, ingredient.count()));
        if assign(rest, cells, taken, used) {
            return true;
        }
        used.pop();
        taken[i] = false;
    }
    false
}
