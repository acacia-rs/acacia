//! Packet builders shared by the tracker tests.

use acacia_client::proto::manual::Uuid;
use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::packets::{CraftingData, InventoryContent, InventorySlot, ItemStackResponse, UpdateTrade};
use acacia_client::proto::types::{
    ContainerSlotType, FullContainerName, ItemExtraDataWithoutBlockingTick, ItemExtraDataWithoutBlockingTickHasNbt,
    ItemExtraDataWithoutBlockingTickNbt, ItemLegacy, ItemLegacyExtra, ItemStackResponsesItem,
    ItemStackResponsesItemContainersItem, ItemStackResponsesItemContainersItemSlotsItem, ItemStackResponsesItemStatus,
    ItemV4, ItemV4Extra, RecipeIngredient, RecipeIngredientContent, RecipeIngredientContentInvalid,
    RecipeIngredientContentValid, RecipeIngredientContentValidContent, RecipeIngredientContentValidContentItemTag,
    RecipeIngredientContentValidContentName, RecipeIngredientType, ShapedRecipe, ShapelessRecipe, WindowID, WindowIDVarint,
    WindowType,
};
use acacia_client::proto::{encode_packet, Packet, RawPacket};
use bytes::{Bytes, BytesMut};

pub fn raw<T: Packet>(packet: &T) -> RawPacket {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    RawPacket::parse(buf.freeze()).unwrap()
}

/// Every sample of `T` in acacia-proto's byte fixtures (one hex-encoded packet per line).
pub fn fixtures<T: Packet>() -> Vec<RawPacket> {
    let path = format!("{}/../acacia-proto/tests/fixtures/packets/{}.hex", env!("CARGO_MANIFEST_DIR"), T::NAME);
    let hex = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    hex.lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let bytes: Vec<u8> = (0..line.len()).step_by(2).map(|i| u8::from_str_radix(&line[i..i + 2], 16).unwrap()).collect();
            RawPacket::parse(Bytes::from(bytes)).unwrap()
        })
        .collect()
}

fn container_name(kind: ContainerSlotType) -> FullContainerName {
    FullContainerName { container_id: kind, dynamic_container_id: None }
}

pub fn content(window: WindowIDVarint, items: Vec<ItemV4>) -> InventoryContent {
    InventoryContent { window_id: window, input: items, container: container_name(ContainerSlotType::AnvilInput), storage_item: empty_item() }
}

pub fn slot_packet(window: WindowIDVarint, slot: u32, item: ItemV4) -> InventorySlot {
    InventorySlot { window_id: window, slot, container: None, storage_item: None, item }
}

/// One `ItemStackResponse` touching a single slot; an error response carries no containers.
pub fn response(ok: bool, kind: ContainerSlotType, slot: u8, count: u8, stack_id: i32, name: &str) -> ItemStackResponse {
    let slots = vec![ItemStackResponsesItemContainersItemSlotsItem {
        slot,
        hotbar_slot: slot,
        count,
        item_stack_id: Some(stack_id),
        custom_name: name.into(),
        filtered_custom_name: String::new(),
        durability_correction: 0,
    }];
    let status = if ok { ItemStackResponsesItemStatus::Ok } else { ItemStackResponsesItemStatus::Error };
    let containers = ok.then(|| vec![ItemStackResponsesItemContainersItem { slot_type: container_name(kind), slots }]);
    ItemStackResponse { responses: vec![ItemStackResponsesItem { status, request_id: -1, containers }] }
}

pub fn item(network_id: i16, count: u16, stack_id: i32) -> ItemV4 {
    ItemV4 {
        network_id,
        count,
        metadata: 0,
        has_stack_id: true,
        stack_id: Some(stack_id),
        block_runtime_id: 0,
        extra: ItemV4Extra::Default(None),
    }
}

pub fn empty_item() -> ItemV4 {
    ItemV4 { has_stack_id: false, stack_id: None, ..item(0, 0, 0) }
}

pub fn legacy_item(network_id: i32, count: u16) -> ItemLegacy {
    ItemLegacy { network_id, count, metadata: 0, block_runtime_id: 0, extra: ItemLegacyExtra::Default(None) }
}

/// A recipe ingredient: `name` with a `#` prefix is an item tag, an empty name a blank cell.
pub fn ingredient(name: &str, count: i32) -> RecipeIngredient {
    use RecipeIngredientContentValidContent as V;
    if name.is_empty() {
        return RecipeIngredient {
            r#type: RecipeIngredientType::Invalid,
            content: RecipeIngredientContent::Invalid(RecipeIngredientContentInvalid { metadata: 0 }),
            count: 0,
        };
    }
    let content = match name.strip_prefix('#') {
        Some(tag) => ("item_tag", V::ItemTag(RecipeIngredientContentValidContentItemTag { tag: tag.into(), metadata: 32767 })),
        None => ("name", V::Name(RecipeIngredientContentValidContentName { name: name.into(), metadata: 32767 })),
    };
    let valid = RecipeIngredientContentValid { descriptor_type: content.0.into(), content: content.1 };
    RecipeIngredient { r#type: RecipeIngredientType::Valid, content: RecipeIngredientContent::Valid(valid), count }
}

pub fn shaped(network_id: u32, (width, height): (i32, i32), input: Vec<RecipeIngredient>, output: ItemLegacy) -> ShapedRecipe {
    ShapedRecipe {
        recipe_id: format!("test:shaped_{network_id}"),
        width,
        height,
        input,
        output: vec![output],
        uuid: Uuid::default(),
        block: "crafting_table".into(),
        priority: 0,
        assume_symmetry: true,
        unlocking_requirement: None,
        network_id,
    }
}

pub fn shapeless(network_id: u32, block: &str, input: Vec<RecipeIngredient>, output: ItemLegacy) -> ShapelessRecipe {
    ShapelessRecipe {
        recipe_id: format!("test:shapeless_{network_id}"),
        input,
        output: vec![output],
        uuid: Uuid::default(),
        block: block.into(),
        priority: 0,
        unlocking_requirement: None,
        network_id,
    }
}

pub fn crafting_data(shaped_recipes: Vec<ShapedRecipe>, shapeless_recipes: Vec<ShapelessRecipe>, clear: bool) -> CraftingData {
    CraftingData {
        shaped_recipes,
        shapeless_recipes,
        multi_recipes: vec![],
        shulker_box_recipes: vec![],
        shapeless_chemistry_recipes: vec![],
        shaped_chemistry_recipes: vec![],
        smithing_transform_recipes: vec![],
        smithing_trim_recipes: vec![],
        potion_type_recipes: vec![],
        potion_container_recipes: vec![],
        material_reducers: vec![],
        clear_recipes: clear,
    }
}

fn compound(entries: Vec<(&str, Value)>) -> Value {
    Value::Compound(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn trade_item(name: &str, count: i8) -> Value {
    compound(vec![("Count", Value::Byte(count)), ("Damage", Value::Short(0)), ("Name", Value::String(name.into()))])
}

/// A villager's trade screen (window 1, trader unique id 77, level 1) with `recipes` as offers.
pub fn update_trade(recipes: Vec<Value>) -> UpdateTrade {
    let offers = Nbt { name: String::new(), value: compound(vec![("Recipes", Value::List(List { tag: 10, items: recipes }))]) };
    UpdateTrade {
        window_id: WindowID::First,
        window_type: WindowType::Trading,
        size: 0,
        trade_tier: 1,
        villager_unique_id: 0,
        entity_unique_id: 77,
        display_name: "entity.villager.librarian".into(),
        new_trading_ui: true,
        economic_trades: true,
        offers: offers.into(),
    }
}

/// One offer of `update_trade`: `(identifier, count)` per item.
pub fn offer(buy_a: (&str, i8), buy_b: Option<(&str, i8)>, sell: (&str, i8), net_id: i32) -> Value {
    let mut entries = vec![("buyA", trade_item(buy_a.0, buy_a.1)), ("buyCountA", Value::Int(i32::from(buy_a.1)))];
    if let Some(b) = buy_b {
        entries.extend([("buyB", trade_item(b.0, b.1)), ("buyCountB", Value::Int(i32::from(b.1)))]);
    }
    entries.extend([
        ("sell", trade_item(sell.0, sell.1)),
        ("uses", Value::Int(0)),
        ("maxUses", Value::Int(12)),
        ("tier", Value::Int(0)),
        ("netId", Value::Int(net_id)),
    ]);
    compound(entries)
}

/// An item whose NBT carries `display.Name` and `display.Lore`.
pub fn named_item(network_id: i16, count: u16, name: &str, lore: &[&str]) -> ItemV4 {
    let lore = List { tag: 8, items: lore.iter().map(|l| Value::String((*l).into())).collect() };
    let display = Value::Compound(vec![("Name".into(), Value::String(name.into())), ("Lore".into(), Value::List(lore))]);
    let nbt = Nbt { name: String::new(), value: Value::Compound(vec![("display".into(), display)]) };
    let extra = ItemExtraDataWithoutBlockingTick {
        has_nbt: ItemExtraDataWithoutBlockingTickHasNbt::True,
        nbt: Some(ItemExtraDataWithoutBlockingTickNbt { version: 1, nbt }),
        can_place_on: vec![],
        can_destroy: vec![],
    };
    ItemV4 { extra: ItemV4Extra::Default(Some(extra)), ..item(network_id, count, 1) }
}
