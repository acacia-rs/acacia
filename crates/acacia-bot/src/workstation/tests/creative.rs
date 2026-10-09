use acacia_client::proto::packets::{
    CreativeContent, CreativeContentGroupsItem, CreativeContentGroupsItemCategory as Category, CreativeContentItemsItem,
};
use acacia_client::proto::types::{ContainerSlotType as T, ItemStackRequestActionsItemContent as Content, ItemStackRequestActionsItemTypeId as TypeId};

use super::*;
use crate::items::ui;
use crate::state::queries::test_support::legacy_item;
use crate::state::CreativeTab;
use crate::workstation::creative::creative_plan;

fn content() -> CreativeContent {
    let group = |category| CreativeContentGroupsItem { category, name: String::new(), icon_item: legacy_item(0, 0) };
    let item = |entry_id, name: &str, group_index| CreativeContentItemsItem { entry_id, item: legacy_item(id(name), 1), group_index };
    CreativeContent {
        groups: vec![group(Category::Construction), group(Category::Items), group(Category::ItemCommandOnly)],
        items: vec![item(1, "minecraft:stone", 0), item(2, "minecraft:stick", 1), item(3, "minecraft:paper", 2)],
    }
}

#[test]
fn creative_items_keep_their_tab_and_take_as_a_craft() {
    let mut state = state(&[]);
    assert!(state.creative.items().is_empty(), "before the server sent them");
    state.apply(&raw(&content())).unwrap();
    let items = state.creative.items();
    assert_eq!(items.iter().map(|i| (i.entry_id, i.tab)).collect::<Vec<_>>(), [(1, CreativeTab::Construction), (2, CreativeTab::Items)], "command-only items are left out");

    let request = encode(&state, &creative_plan(&state, 2, 64).unwrap());
    assert_eq!(request.actions.iter().map(|a| a.type_id).collect::<Vec<_>>(), [TypeId::CraftCreative, TypeId::ResultsDeprecated, TypeId::Place]);
    let Content::CraftCreative(craft) = &request.actions[0].content else { panic!() };
    assert_eq!((craft.item_id, craft.times_crafted), (2, 1));
    let Content::Place(place) = &request.actions[2].content else { panic!() };
    assert_eq!((place.count, place.source.clone()), (64, info(T::CreativeOutput, ui::CREATED_OUTPUT, REQUEST)));
    assert!(creative_plan(&state, 3, 1).is_err(), "not offered");
}
