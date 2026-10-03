use acacia_client::proto::nbt::Value;
use acacia_client::proto::packets::PlayerEnchantOptions;
use acacia_client::proto::types::{
    ContainerSlotType as T, EnchantOption, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId, WindowType,
};

use super::*;
use crate::items::ui;
use crate::workstation::enchant::enchant_plan;
use crate::workstation::loom::{dye_color, loom_plan};
use crate::ActionError;

fn put_ui(state: &mut GameState, slot: u8, item: ItemStack) {
    state.inventory.ui[usize::from(slot)] = item;
}

fn enchanting_state(xp_level: i32) -> GameState {
    let mut state = state(&[]);
    open(&mut state, WindowType::Enchantment);
    put_ui(&mut state, ui::ENCHANTING_INPUT, stack("minecraft:diamond_sword", 1, 31));
    put_ui(&mut state, ui::ENCHANTING_LAPIS, stack("minecraft:lapis_lazuli", 3, 32));
    let option = |cost, option_id| EnchantOption {
        cost,
        slot_flags: 0,
        equip_enchants: vec![],
        held_enchants: vec![],
        self_enchants: vec![],
        name: String::new(),
        option_id,
    };
    state.apply(&raw(&PlayerEnchantOptions { options: vec![option(3, 100_000), option(8, 100_001), option(15, 100_002)] })).unwrap();
    state.player.xp_level = xp_level;
    state
}

#[test]
fn enchanting_crafts_the_option_and_puts_the_item_back() {
    let state = enchanting_state(10);
    let request = encode(&state, &enchant_plan(&state, 1).unwrap());
    // Request -53 of the 2026-10-02 capture: the lapis goes last, the emptied input by the request id.
    assert_eq!(kinds(&request), [(TypeId::CraftRecipe, 12), (TypeId::ResultsDeprecated, 19), (TypeId::Consume, 5), (TypeId::Place, 1), (TypeId::Consume, 5)]);
    let Content::CraftRecipe(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.recipe_network_id, craft.times_crafted), (100_001, 1));
    let Content::Place(place) = &request.actions[3].content else { panic!() };
    assert_eq!(place.destination, info(T::EnchantingInput, ui::ENCHANTING_INPUT, REQUEST));
    let Content::Consume(lapis) = &request.actions[4].content else { panic!() };
    assert_eq!((lapis.count, lapis.source.clone()), (2, info(T::EnchantingLapis, ui::ENCHANTING_LAPIS, 32)));

    assert!(matches!(enchant_plan(&state, 2), Err(ActionError::NotPossible(_))), "level 15 needed");
    assert!(matches!(enchant_plan(&enchanting_state(30), 3), Err(ActionError::NotPossible(_))), "three options");
}

#[test]
fn loom_result_carries_the_new_pattern() {
    let mut state = state(&[]);
    open(&mut state, WindowType::Loom);
    let banner_nbt = Nbt { name: String::new(), value: Value::Compound(vec![("Type".into(), Value::Int(0))]) };
    put_ui(&mut state, ui::LOOM_BANNER, ItemStack { nbt: Some(banner_nbt), ..stack("minecraft:white_banner", 1, 61) });
    put_ui(&mut state, ui::LOOM_DYE, stack("minecraft:red_dye", 5, 62));
    let request = encode(&state, &loom_plan(&state, "bo").unwrap());
    assert_eq!(kinds(&request)[0], (TypeId::CraftLoomRequest, 17));
    let Content::ResultsDeprecated(results) = &request.actions[1].content else { panic!() };
    let nbt = &results.result_items[0].extra.as_ref().unwrap().nbt.as_ref().unwrap().nbt;
    let Value::Compound(entries) = &nbt.value else { panic!() };
    assert_eq!(entries.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["Patterns", "Type"], "vanilla's key order");
    let Some(Value::List(patterns)) = nbt.value.get("Patterns") else { panic!("{nbt:?}") };
    let layer = &patterns.items[0];
    assert_eq!((layer.get("Pattern"), layer.get("Color")), (Some(&Value::String("bo".into())), Some(&Value::Int(1))));
    assert_eq!((dye_color("minecraft:ink_sac"), dye_color("minecraft:white_dye"), dye_color("minecraft:stone")), (Some(0), Some(15), None));
}
