use acacia_client::proto::packets::{CraftingData, ItemStackResponse};
use acacia_client::proto::types::{
    ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId,
    ItemStackResponsesItem, ItemStackResponsesItemContainersItem, ItemStackResponsesItemContainersItemSlotsItem,
    ItemStackResponsesItemStatus, RecipeIngredient2Content, SmithingTransformRecipe, WindowType,
};

use super::*;
use crate::items::{ui, SlotRef};
use crate::state::queries::test_support::{crafting_data, ingredient, legacy_item, shaped, shapeless};
use crate::workstation::craft::{auto_craft_plan, book_crafts, pick_recipe};
use crate::workstation::smithing::{smithing_plan, stonecut_plan};
use crate::ActionError;

pub(super) const STICKS: u32 = 21;
pub(super) const TABLE: u32 = 22;
const FENCE: u32 = 659;
const SLABS: u32 = 30;
const NETHERITE: u32 = 40;

pub(super) fn recipes() -> CraftingData {
    let planks = || ingredient("#minecraft:planks", 1);
    let sticks = shaped(STICKS, (1, 2), vec![planks(), planks()], legacy_item(id("minecraft:stick"), 4));
    let table = shaped(TABLE, (3, 3), (0..9).map(|_| planks()).collect(), legacy_item(id("minecraft:crafting_table"), 1));
    let (oak, stick) = (|| ingredient("minecraft:oak_planks", 1), || ingredient("minecraft:stick", 1));
    let fence = shaped(FENCE, (3, 2), vec![oak(), stick(), oak(), oak(), stick(), oak()], legacy_item(id("minecraft:oak_fence"), 3));
    let slabs = shapeless(SLABS, "stonecutter", vec![ingredient("minecraft:stone", 1)], legacy_item(id("minecraft:stone_slab"), 2));
    let mut data = crafting_data(vec![sticks, table, fence], vec![slabs], true);
    data.smithing_transform_recipes = vec![SmithingTransformRecipe {
        recipe_id: "minecraft:smithing_netherite_sword".into(),
        template: ingredient("minecraft:netherite_upgrade_smithing_template", 1),
        base: ingredient("minecraft:diamond_sword", 1),
        addition: ingredient("minecraft:netherite_ingot", 1),
        result: legacy_item(id("minecraft:netherite_sword"), 1),
        tag: "smithing_table".into(),
        network_id: NETHERITE,
    }];
    data
}

pub(super) fn with_recipes(main: &[(usize, ItemStack)]) -> GameState {
    let mut state = state(main);
    state.apply(&raw(&recipes())).unwrap();
    state
}

#[test]
fn recipe_book_craft_consumes_per_cell_and_places_the_result() {
    let state = with_recipes(&[(0, stack("minecraft:oak_planks", 10, 7))]);
    let recipe = pick_recipe(&state, "minecraft:stick", false).unwrap();
    assert_eq!(recipe.network_id, STICKS);
    let plan = auto_craft_plan(&state, recipe, 1).unwrap();
    let request = encode(&state, &plan);

    assert_eq!(kinds(&request), [
        (TypeId::CraftRecipeAuto, 13),
        (TypeId::ResultsDeprecated, 19),
        (TypeId::Consume, 5),
        (TypeId::Consume, 5),
        (TypeId::Place, 1),
    ]);
    let Content::CraftRecipeAuto(auto) = &request.actions[0].content else { panic!() };
    assert_eq!((auto.recipe_network_id, auto.times_crafted, auto.ingredients.len()), (STICKS, 1, 2));
    // BDS rejects tag descriptors here: each tag cell names the item it uses up.
    for cell in &auto.ingredients {
        assert!(matches!(&cell.content, RecipeIngredient2Content::Name(n) if n.name == "minecraft:oak_planks" && n.metadata == 0), "{cell:?}");
        assert_eq!(cell.count, 1);
    }
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    assert_eq!((results.times_crafted, results.result_items[0].count), (1, 4));
    assert_eq!(results.result_items[0].content.as_ref().unwrap().name, "minecraft:stick");
    // Both cells draw on the same stack; once changed, it goes by the request id.
    let consumed: Vec<_> = request.actions[2..4].iter().map(|a| if let Content::Consume(c) = &a.content { (c.count, c.source.clone()) } else { panic!() }).collect();
    assert_eq!(consumed, [(1, info(T::HotbarAndInventory, 0, 7)), (1, info(T::HotbarAndInventory, 0, REQUEST))]);
    let Content::Place(place) = &request.actions[4].content else { panic!() };
    assert_eq!(place.count, 4);
    assert_eq!(place.source, info(T::CreativeOutput, ui::CREATED_OUTPUT, REQUEST), "the result is referred to by the request id");
    assert_eq!(place.destination, info(T::HotbarAndInventory, 1, 0));
    assert!(request.custom_names.is_empty());
}

/// Request -31 of the 2026-10-02 capture: two oak fences from the recipe book at a table.
#[test]
fn recipe_book_craft_all_matches_vanilla() {
    let mut state = with_recipes(&[(0, stack("minecraft:oak_planks", 8, 67)), (33, stack("minecraft:stick", 4, 71))]);
    open(&mut state, WindowType::Workbench);
    let recipe = pick_recipe(&state, "minecraft:oak_fence", true).unwrap().clone();
    assert_eq!((book_crafts(&state, &recipe, 6), book_crafts(&state, &recipe, 3)), (2, 1), "craft all only when all is wanted");
    let request = encode(&state, &auto_craft_plan(&state, &recipe, 2).unwrap());
    let Content::CraftRecipeAuto(auto) = &request.actions[0].content else { panic!() };
    assert_eq!((auto.recipe_network_id, auto.times_crafted), (FENCE, 2));
    let cells: Vec<_> = auto.ingredients.iter().map(|i| if let RecipeIngredient2Content::Name(n) = &i.content { (n.name.as_str(), n.metadata, i.count) } else { panic!() }).collect();
    let (p, s) = (("minecraft:oak_planks", 32767, 1), ("minecraft:stick", 32767, 1));
    assert_eq!(cells, [p, s, p, p, s, p]);
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    assert_eq!((results.times_crafted, results.result_items[0].count), (2, 3));
    let hai = |slot, id| info(T::HotbarAndInventory, slot, id);
    let consumed: Vec<_> = request.actions[2..8].iter().map(|a| if let Content::Consume(c) = &a.content { (c.count, c.source.clone()) } else { panic!() }).collect();
    let r = REQUEST;
    assert_eq!(consumed, [(2, hai(0, 67)), (2, hai(33, 71)), (2, hai(0, r)), (2, hai(0, r)), (2, hai(33, r)), (2, hai(0, r))]);
    let Content::Place(place) = &request.actions[8].content else { panic!() };
    assert_eq!((place.count, place.source.clone(), place.destination.clone()), (6, info(T::CreativeOutput, ui::CREATED_OUTPUT, r), hai(0, r)));
}

#[test]
fn accepted_craft_updates_the_inventory() {
    let mut state = with_recipes(&[(0, stack("minecraft:oak_planks", 10, 7))]);
    let (craft, ops) = auto_craft_plan(&state, pick_recipe(&state, "minecraft:stick", false).unwrap(), 1).unwrap();
    let plan = Plan::craft(&state, Screen::of(&state, BlockKind::Other), &craft, &ops).unwrap();
    let slot = |kind, slot, count, stack_id| ItemStackResponsesItemContainersItem {
        slot_type: FullContainerName { container_id: kind, dynamic_container_id: None },
        slots: vec![ItemStackResponsesItemContainersItemSlotsItem {
            slot,
            hotbar_slot: slot,
            count,
            item_stack_id: Some(stack_id),
            custom_name: String::new(),
            filtered_custom_name: String::new(),
            durability_correction: 0,
        }],
    };
    let response = ItemStackResponsesItem {
        status: ItemStackResponsesItemStatus::Ok,
        request_id: REQUEST,
        containers: Some(vec![slot(T::Hotbar, 0, 8, 7), slot(T::Hotbar, 1, 4, 55)]),
    };
    state.apply(&raw(&ItemStackResponse { responses: vec![response.clone()] })).unwrap();
    plan.commit(&mut state, &response);
    assert_eq!(state.inventory.main[0].count, 8);
    assert_eq!(state.inventory.main[1], stack("minecraft:stick", 4, 55));
    assert!(state.inventory.ui[usize::from(ui::CREATED_OUTPUT)].is_empty());
}

#[test]
fn recipe_choice_respects_grid_and_stock() {
    let state = with_recipes(&[(3, stack("minecraft:oak_planks", 8, 7))]);
    assert!(matches!(pick_recipe(&state, "minecraft:crafting_table", false), Err(ActionError::NotPossible(_))), "3x3 needs a table");
    assert!(matches!(pick_recipe(&state, "minecraft:crafting_table", true), Err(ActionError::NotPossible(_))), "8 of 9 planks");
    let state = with_recipes(&[(3, stack("minecraft:oak_planks", 5, 7)), (20, stack("minecraft:oak_planks", 4, 8))]);
    let plan = auto_craft_plan(&state, pick_recipe(&state, "minecraft:crafting_table", true).unwrap(), 1).unwrap();
    let consumed: Vec<_> = plan.1.iter().filter_map(|op| if let Op::Consume { from, count } = op { Some((*from, *count)) } else { None }).collect();
    assert_eq!(consumed.len(), 9);
    assert_eq!(consumed.iter().filter(|(from, _)| *from == SlotRef::Main(20)).count(), 4, "the second stack covers the last cells");
}

#[test]
fn stonecutter_cuts_several_at_once() {
    let mut state = with_recipes(&[]);
    open(&mut state, WindowType::Stonecutter);
    state.inventory.ui[usize::from(ui::STONECUTTER_INPUT)] = stack("minecraft:stone", 10, 12);
    let recipe = state.recipes.book().get(SLABS).unwrap().clone();
    let request = encode(&state, &stonecut_plan(&state, &recipe, 7).unwrap());
    // No CraftResultsDeprecated: BDS crashes on it in a stonecutter request.
    assert_eq!(kinds(&request), [(TypeId::CraftRecipe, 12), (TypeId::Consume, 5), (TypeId::Place, 1)]);
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (SLABS, 4));
    let Content::Consume(consume) = &request.actions[1].content else { panic!() };
    assert_eq!((consume.count, consume.source.clone()), (4, info(T::StonecutterInput, ui::STONECUTTER_INPUT, 12)));
    let Content::Place(place) = &request.actions[2].content else { panic!() };
    assert_eq!((place.count, place.destination.clone()), (8, info(T::HotbarAndInventory, 0, 0)));
}

#[test]
fn smithing_upgrade_consumes_all_three_slots() {
    let mut state = with_recipes(&[]);
    open(&mut state, WindowType::SmithingTable);
    for (slot, name) in [
        (ui::SMITHING_INPUT, "minecraft:diamond_sword"),
        (ui::SMITHING_MATERIAL, "minecraft:netherite_ingot"),
        (ui::SMITHING_TEMPLATE, "minecraft:netherite_upgrade_smithing_template"),
    ] {
        state.inventory.ui[usize::from(slot)] = stack(name, 1, i32::from(slot));
    }
    let request = encode(&state, &smithing_plan(&state).unwrap());
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!(craft.recipe_network_id, NETHERITE);
    let sources: Vec<_> = request.actions[2..5]
        .iter()
        .map(|a| if let Content::Consume(c) = &a.content { c.source.slot_type.container_id } else { panic!() })
        .collect();
    assert_eq!(sources, [T::SmithingTableTemplate, T::SmithingTableInput, T::SmithingTableMaterial], "vanilla's order");
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    assert_eq!(results.result_items[0].content.as_ref().unwrap().name, "minecraft:netherite_sword");
}
