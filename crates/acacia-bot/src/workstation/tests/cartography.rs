use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::packets::CraftingData;
use acacia_client::proto::types::{
    ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId,
    ItemStackRequestCause, MultiRecipe, WindowType,
};

use super::*;
use crate::items::craft::CraftAction;
use crate::items::{ui, SlotRef};
use crate::state::queries::test_support::crafting_data;
use crate::workstation::cartography::cartography_plan;
use crate::ActionError;

const CLONE: u32 = 4760;
const EXTEND: u32 = 4762;

/// A cartography table screen whose server sent the map multi recipes, as the captured BDS did.
fn table() -> GameState {
    let mut state = state(&[]);
    let multi = |uuid: &str, network_id| MultiRecipe { uuid: uuid.parse().unwrap(), network_id };
    let data = CraftingData {
        multi_recipes: vec![multi("442d85ed-8272-4543-a6f1-418f90ded05d", CLONE), multi("8b36268c-1829-483c-a0f1-993b7156a8f2", EXTEND)],
        ..crafting_data(vec![], vec![], true)
    };
    state.apply(&raw(&data)).unwrap();
    open(&mut state, WindowType::Cartography);
    state
}

fn map(count: u16, stack_id: i32) -> ItemStack {
    let nbt = Value::Compound(vec![("map_name_index".into(), Value::Int(1)), ("map_uuid".into(), Value::Long(-12884901870))]);
    ItemStack { count, nbt: Some(Nbt { name: String::new(), value: nbt }), ..stack("minecraft:filled_map", count, stack_id) }
}

fn put(state: &mut GameState, slot: u8, item: ItemStack) {
    state.inventory.ui[usize::from(slot)] = item;
}

#[test]
fn clone_is_an_optional_craft_with_an_empty_name() {
    let mut state = table();
    put(&mut state, ui::CARTOGRAPHY_INPUT, map(1, 85));
    put(&mut state, ui::CARTOGRAPHY_ADDITIONAL, stack("minecraft:empty_map", 1, 84));
    let request = encode(&state, &cartography_plan(&state, None).unwrap());
    // The captured -129: no CraftResultsDeprecated, both inputs consumed, two maps out.
    assert_eq!(kinds(&request), [(TypeId::Optional, 15), (TypeId::Consume, 5), (TypeId::Consume, 5), (TypeId::Place, 1)]);
    let Content::Optional(optional) = &request.actions[0].content else { panic!() };
    assert_eq!((optional.recipe_network_id, optional.filtered_string_index), (CLONE, 0));
    let Content::Consume(additional) = &request.actions[2].content else { panic!() };
    assert_eq!(additional.source, info(T::CartographyAdditional, ui::CARTOGRAPHY_ADDITIONAL, 84));
    let Content::Place(place) = &request.actions[3].content else { panic!() };
    assert_eq!((place.count, place.source.clone()), (2, info(T::CreativeOutput, 50, REQUEST)));
    assert_eq!(place.destination, info(T::HotbarAndInventory, 0, 0));
    assert_eq!((request.custom_names.clone(), request.cause), (vec![String::new()], ItemStackRequestCause::CartographyText));
}

#[test]
fn rename_takes_one_map_per_craft() {
    let mut state = table();
    put(&mut state, ui::CARTOGRAPHY_INPUT, map(2, 86));
    let (craft, ops) = cartography_plan(&state, Some("2")).unwrap();
    let request = encode(&state, &(craft.clone(), ops));
    assert_eq!(kinds(&request), [(TypeId::Optional, 15), (TypeId::Consume, 5), (TypeId::Place, 1)]);
    let Content::Consume(consume) = &request.actions[1].content else { panic!() };
    assert_eq!((consume.count, consume.source.clone()), (1, info(T::CartographyInput, ui::CARTOGRAPHY_INPUT, 86)));
    assert_eq!(request.custom_names, ["2"]);
    let result = &craft.results[0].1;
    assert_eq!(result.count, 1);
    let display = result.nbt.as_ref().unwrap().value.get("display").unwrap();
    assert_eq!(display.get("Name"), Some(&Value::String("2".into())));
}

#[test]
fn zoom_uses_the_extend_recipe_and_needs_the_server_recipes() {
    let mut screen = table();
    put(&mut screen, ui::CARTOGRAPHY_INPUT, map(1, 85));
    assert!(matches!(cartography_plan(&screen, None), Err(ActionError::NotPossible(_))), "nothing to craft");
    put(&mut screen, ui::CARTOGRAPHY_ADDITIONAL, stack("minecraft:paper", 3, 90));
    let (craft, ops) = cartography_plan(&screen, None).unwrap();
    assert_eq!(craft.action, CraftAction::Optional { network_id: EXTEND, filter_index: 0 });
    assert_eq!(ops[1], Op::Consume { from: SlotRef::Ui(ui::CARTOGRAPHY_ADDITIONAL), count: 1 });

    let mut bare = state(&[]);
    open(&mut bare, WindowType::Cartography);
    put(&mut bare, ui::CARTOGRAPHY_INPUT, map(1, 85));
    put(&mut bare, ui::CARTOGRAPHY_ADDITIONAL, stack("minecraft:empty_map", 1, 84));
    assert!(matches!(cartography_plan(&bare, None), Err(ActionError::NotPossible(_))));
}
