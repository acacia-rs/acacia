use acacia_client::proto::types::{ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId, WindowType};

use super::crafting::{with_recipes, STICKS, TABLE};
use super::*;
use crate::items::SlotRef;
use crate::workstation::held_grid::{held, held_grid_plan};

fn planks(count: u16, stack_id: i32) -> ItemStack {
    stack("minecraft:oak_planks", count, stack_id)
}

fn matched_cells(state: &GameState) -> Option<(u32, Vec<(SlotRef, u16)>)> {
    held(state).map(|h| (h.recipe.network_id, h.cells))
}

#[test]
fn a_shaped_recipe_matches_anywhere_in_the_grid() {
    let mut state = with_recipes(&[]);
    // the right column of the 2x2 grid (28 29 / 30 31)
    state.inventory.ui[29] = planks(3, 50);
    state.inventory.ui[31] = planks(2, 51);
    assert_eq!(matched_cells(&state), Some((STICKS, vec![(SlotRef::Ui(29), 1), (SlotRef::Ui(31), 1)])));
}

#[test]
fn stray_cells_or_a_wrong_shape_match_nothing() {
    let mut state = with_recipes(&[]);
    state.inventory.ui[28] = planks(1, 50);
    assert!(held(&state).is_none(), "one plank is no recipe");
    state.inventory.ui[29] = planks(1, 51);
    assert!(held(&state).is_none(), "a row of two is not the stick column");
    state.inventory.ui[30] = planks(1, 52);
    assert!(held(&state).is_none(), "a stray cell beside the column");
}

#[test]
fn the_table_grid_is_used_while_a_table_is_open() {
    let mut state = with_recipes(&[]);
    open(&mut state, WindowType::Workbench);
    for (i, cell) in (32..=40).enumerate() {
        state.inventory.ui[cell] = planks(1, 60 + i as i32);
    }
    assert_eq!(matched_cells(&state).map(|(id, cells)| (id, cells.len())), Some((TABLE, 9)));
}

#[test]
fn a_click_crafts_once_onto_the_cursor_and_a_shift_click_all_into_the_inventory() {
    let mut state = with_recipes(&[]);
    state.inventory.ui[28] = planks(3, 50);
    state.inventory.ui[30] = planks(2, 51);

    let request = encode(&state, &held_grid_plan(&state, 1, true).unwrap());
    assert_eq!(kinds(&request).iter().map(|k| k.0).collect::<Vec<_>>(), [
        TypeId::CraftRecipe,
        TypeId::ResultsDeprecated,
        TypeId::Consume,
        TypeId::Consume,
        TypeId::Take,
    ]);
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (STICKS, 1));
    let Content::Take(take) = &request.actions[4].content else { panic!() };
    assert_eq!(take.count, 4);

    let (craft, _) = held_grid_plan(&state, u8::MAX, false).unwrap();
    assert_eq!(craft.action.times(), 2, "the shorter cell allows two crafts");
}
