use acacia_client::proto::types::{ItemStackRequestActionsItemContent as Content, WindowType};

use super::crafting::{with_recipes, STICKS, TABLE};
use super::*;
use crate::items::{Op, SlotRef};
use crate::workstation::hand::{grid_match, hand_craft_plan};

fn planks(count: u16, stack_id: i32) -> ItemStack {
    stack("minecraft:oak_planks", count, stack_id)
}

#[test]
fn a_shaped_recipe_is_found_anywhere_in_the_grid() {
    let mut state = with_recipes(&[]);
    assert!(grid_match(&state, false).is_none(), "an empty grid");
    // The right column of the 2x2 grid.
    state.inventory.ui[29] = planks(5, 50);
    state.inventory.ui[31] = planks(3, 51);
    let (recipe, used) = grid_match(&state, false).unwrap();
    assert_eq!((recipe.network_id, used), (STICKS, vec![(SlotRef::Ui(29), 1), (SlotRef::Ui(31), 1)]));

    let request = encode(&state, &hand_craft_plan(&state, false, true).unwrap());
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (STICKS, 3), "as many crafts as the smaller cell holds");
    let consumed: Vec<_> = request.actions[2..4].iter().map(|a| if let Content::Consume(c) = &a.content { (c.count, c.source.slot) } else { panic!() }).collect();
    assert_eq!(consumed, [(3, 29), (3, 31)]);
    let Content::CraftRecipe(craft) = &encode(&state, &hand_craft_plan(&state, false, false).unwrap()).actions[0].content else { panic!() };
    assert_eq!(craft.times_crafted, 1, "a plain click crafts once");
    let (craft, ops) = hand_craft_plan(&state, false, false).unwrap();
    let taken = crate::items::onto_cursor(&state, &craft.created(), ops.clone());
    assert_eq!(taken.last(), Some(&Op::Transfer { from: SlotRef::CREATED_OUTPUT, to: SlotRef::Cursor, count: 4 }), "onto the empty cursor");
    assert_eq!(taken.iter().filter(|op| matches!(op, Op::Transfer { .. })).count(), 1);
    state.inventory.ui[0] = planks(1, 70);
    assert_eq!(crate::items::onto_cursor(&state, &craft.created(), ops.clone()), ops, "the cursor holds something else");
    state.inventory.ui[0] = ItemStack::default();

    state.inventory.ui[31] = ItemStack::default();
    state.inventory.ui[28] = planks(3, 51);
    assert!(grid_match(&state, false).is_none(), "side by side makes nothing");
}

#[test]
fn the_table_grid_matches_its_own_cells() {
    let mut state = with_recipes(&[]);
    open(&mut state, WindowType::Workbench);
    // The middle and bottom cells of the last column.
    state.inventory.ui[37] = planks(1, 50);
    state.inventory.ui[40] = planks(1, 51);
    assert_eq!(grid_match(&state, true).unwrap().0.network_id, STICKS);
    assert!(grid_match(&state, false).is_none(), "the 2x2 grid is empty");
    for cell in 32..=40 {
        state.inventory.ui[cell] = planks(1, 60 + cell as i32);
    }
    assert_eq!(grid_match(&state, true).unwrap().0.network_id, TABLE);
}
