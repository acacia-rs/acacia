use acacia_client::proto::types::{
    ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId, WindowType,
};

use super::crafting::{recipes, with_recipes, STICKS};
use super::*;
use crate::items::{ui, SlotRef};
use crate::state::queries::test_support::{ingredient, legacy_item, shapeless};
use crate::state::Recipe;
use crate::workstation::craft::{auto_craft_plan, pick_recipe};
use crate::workstation::grid::{grid_batch, grid_craft_plan, grid_layout, placement_clicks};
use crate::ActionError;

const TWO_RESULTS: u32 = 50;

fn planks(count: u16, stack_id: i32) -> ItemStack {
    stack("minecraft:oak_planks", count, stack_id)
}

#[test]
fn grid_craft_lays_out_then_shift_clicks_all_crafts() {
    let mut state = with_recipes(&[(0, planks(10, 7))]);
    let recipe = pick_recipe(&state, "minecraft:stick", false).unwrap().clone();
    let (times, clicks) = grid_batch(&state, &recipe, false, 8).unwrap();
    assert_eq!(times, 2, "two crafts of four sticks");
    assert_eq!(clicks, [(SlotRef::Main(0), SlotRef::Ui(28), 2), (SlotRef::Main(0), SlotRef::Ui(30), 2)], "a 1x2 recipe fills the left column");
    // Like vanilla's sticks: the stack goes into the cursor, a few per cell, the rest back.
    let cursor = SlotRef::Cursor;
    assert_eq!(placement_clicks(&state, &clicks), [
        Op::Transfer { from: SlotRef::Main(0), to: cursor, count: 10 },
        Op::Transfer { from: cursor, to: SlotRef::Ui(28), count: 2 },
        Op::Transfer { from: cursor, to: SlotRef::Ui(30), count: 2 },
        Op::Transfer { from: cursor, to: SlotRef::Main(0), count: 6 },
    ]);

    state.inventory.main[0].count = 6;
    state.inventory.ui[28] = planks(2, 50);
    state.inventory.ui[30] = planks(2, 51);
    let request = encode(&state, &grid_craft_plan(&state, &recipe, false, 2).unwrap());
    assert_eq!(kinds(&request), [
        (TypeId::CraftRecipe, 12),
        (TypeId::ResultsDeprecated, 19),
        (TypeId::Consume, 5),
        (TypeId::Consume, 5),
        (TypeId::Place, 1),
    ]);
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (STICKS, 2));
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    assert_eq!((results.times_crafted, results.result_items[0].count), (1, 4), "per-craft result, timesCrafted 1 (Geyser #5290)");
    let consumed: Vec<_> = request.actions[2..4].iter().map(|a| if let Content::Consume(c) = &a.content { (c.count, c.source.clone()) } else { panic!() }).collect();
    assert_eq!(consumed, [(2, info(T::CraftingInput, 28, 50)), (2, info(T::CraftingInput, 30, 51))]);
    let Content::Place(place) = &request.actions[4].content else { panic!() };
    assert_eq!((place.count, place.source.clone(), place.destination.clone()), (8, info(T::CreativeOutput, ui::CREATED_OUTPUT, REQUEST), info(T::HotbarAndInventory, 1, 0)));
}

#[test]
fn table_grid_takes_cells_from_several_stacks() {
    let mut state = with_recipes(&[(3, planks(5, 7)), (20, planks(4, 8))]);
    open(&mut state, WindowType::Workbench);
    let sticks = pick_recipe(&state, "minecraft:stick", true).unwrap().clone();
    let cells: Vec<SlotRef> = grid_layout(&sticks, true).unwrap().into_iter().map(|(cell, _)| cell).collect();
    assert_eq!(cells, [SlotRef::Ui(32), SlotRef::Ui(35)]);

    let table = pick_recipe(&state, "minecraft:crafting_table", true).unwrap().clone();
    let (times, clicks) = grid_batch(&state, &table, true, 1).unwrap();
    assert_eq!((times, clicks.len()), (1, 9));
    assert_eq!((clicks[0], clicks[8]), ((SlotRef::Main(3), SlotRef::Ui(32), 1), (SlotRef::Main(20), SlotRef::Ui(40), 1)));
    assert_eq!(clicks.iter().filter(|(from, ..)| *from == SlotRef::Main(20)).count(), 4);
    assert_eq!(placement_clicks(&state, &clicks).len(), 1 + 5 + 1 + 4, "two stacks used up through the cursor");
    let single = [(SlotRef::Main(3), SlotRef::Ui(32), 5)];
    assert_eq!(placement_clicks(&state, &single), [Op::Transfer { from: SlotRef::Main(3), to: SlotRef::Ui(32), count: 5 }], "a whole stack is one click");
    let packet = Plan::build(&state, Screen::of(&state, BlockKind::Other), &placement_clicks(&state, &single)).unwrap().request(REQUEST);
    let Content::Place(place) = &packet.requests[0].actions[0].content else { panic!() };
    assert_eq!(place.source, info(T::HotbarAndInventory, 3, 7), "named like vanilla's logs into the grid");

    for (i, cell) in (ui::CRAFTING_3X3..ui::CRAFTING_3X3 + 9).enumerate() {
        state.inventory.ui[usize::from(cell)] = planks(1, 60 + i as i32);
    }
    assert!(matches!(grid_batch(&state, &table, true, 1), Err(ActionError::NotPossible(_))), "the grid must start empty");
    let request = encode(&state, &grid_craft_plan(&state, &table, true, 1).unwrap());
    assert_eq!(request.actions.len(), 2 + 9 + 1);
    let Content::Place(place) = &request.actions[11].content else { panic!() };
    assert_eq!(place.destination, info(T::HotbarAndInventory, 0, 0), "a container is open");
}

/// A grid recipe turning one stone into a stick and a paper.
fn two_results(main: &[(usize, ItemStack)]) -> (GameState, Recipe) {
    let mut state = state(main);
    let mut data = recipes();
    let mut recipe = shapeless(TWO_RESULTS, "crafting_table", vec![ingredient("minecraft:stone", 1)], legacy_item(id("minecraft:stick"), 1));
    recipe.output.push(legacy_item(id("minecraft:paper"), 1));
    data.shapeless_recipes.push(recipe);
    state.apply(&raw(&data)).unwrap();
    let recipe = state.recipes.book().get(TWO_RESULTS).unwrap().clone();
    (state, recipe)
}

/// Every result gets a `Create`, then its own move out of the created-output slot (Geyser #3682)
/// to `destinations` (slot, stack id).
fn assert_created_one_by_one(request: &ItemStackRequest, from: usize, destinations: [(u8, i32); 2]) {
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    let names: Vec<_> = results.result_items.iter().map(|r| r.content.as_ref().unwrap().name.as_str()).collect();
    assert_eq!(names, ["minecraft:stick", "minecraft:paper"]);
    for (i, (slot, stack_id)) in destinations.into_iter().enumerate() {
        let Content::Create(create) = &request.actions[from + 2 * i].content else { panic!() };
        assert_eq!(create.result_slot_id, i as u8);
        let Content::Place(place) = &request.actions[from + 2 * i + 1].content else { panic!() };
        assert_eq!((place.source.clone(), place.destination.clone()), (info(T::CreativeOutput, ui::CREATED_OUTPUT, REQUEST), info(T::HotbarAndInventory, slot, stack_id)));
    }
}

#[test]
fn several_results_are_created_one_by_one() {
    let (state, recipe) = two_results(&[(0, stack("minecraft:stone", 1, 7))]);
    let request = encode(&state, &auto_craft_plan(&state, &recipe, 1).unwrap());
    assert_eq!(kinds(&request), [
        (TypeId::CraftRecipeAuto, 13),
        (TypeId::ResultsDeprecated, 19),
        (TypeId::Consume, 5),
        (TypeId::Create, 6),
        (TypeId::Place, 1),
        (TypeId::Create, 6),
        (TypeId::Place, 1),
    ]);
    // The first result takes the slot the stone was consumed from (named by the request id from then on).
    assert_created_one_by_one(&request, 3, [(0, REQUEST), (1, 0)]);

    let (mut state, recipe) = two_results(&[(0, stack("minecraft:paper", 1, 7))]);
    state.inventory.ui[usize::from(ui::CRAFTING_2X2)] = stack("minecraft:stone", 1, 8);
    let request = encode(&state, &grid_craft_plan(&state, &recipe, false, 1).unwrap());
    assert_eq!(request.actions[0].type_id, TypeId::CraftRecipe);
    assert_created_one_by_one(&request, 3, [(1, 0), (2, 0)]);
}
