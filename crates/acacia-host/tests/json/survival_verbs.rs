//! Tools, auto-eat, beds, signs, the craftable list and the cursor ray, with FakeServer playing the bed and the sign
//! editor by hand.

use acacia_api::{BlockAtCursorParams, BlockPos, BotId, Face};
use acacia_bot::proto::packets::{BlockEntityData, InventoryTransaction, OpenSign, PlayerAction, SetEntityData, StartGame};
use acacia_bot::proto::types::{
    Action, BlockCoordinates, EntityProperties, MetadataDictionaryItem, MetadataDictionaryItemKey as Key,
    MetadataDictionaryItemType as Type, MetadataDictionaryItemValue as Value, MetadataDictionaryItemValueDefault,
    TransactionTransactionData as Data, TransactionUseItemActionType as UseItem,
};
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;

use super::common::{code, Driver};

/// acacia-bot's `PLAYER_FLAG_SLEEP` (BDS's player flags byte).
const SLEEPING: i8 = 1 << 1;

fn decoded<T: Packet>(server: &FakeServer) -> Vec<T> {
    server.received().into_iter().filter(|r| r.packet.id == T::ID).filter_map(|r| r.packet.decode().ok()).collect()
}

fn clicked(server: &FakeServer, cell: [i32; 3]) -> bool {
    decoded::<InventoryTransaction>(server).into_iter().any(|t| match t.transaction.transaction_data {
        Data::ItemUse(u) => u.action_type == UseItem::ClickBlock && [u.block_position.x, u.block_position.y, u.block_position.z] == cell,
        _ => false,
    })
}

fn acted(server: &FakeServer, action: Action) -> bool {
    decoded::<PlayerAction>(server).iter().any(|p| p.action == action)
}

/// The bot's own player flags as BDS sends them.
fn player_flags(server: &FakeServer, flags: i8) -> SetEntityData {
    let start: StartGame = server.sent().iter().find(|r| r.packet.id == StartGame::ID).and_then(|r| r.packet.decode().ok()).unwrap();
    let value = Value::Default(MetadataDictionaryItemValueDefault::Byte(flags));
    let item = MetadataDictionaryItem { key: Key::PlayerFlags, r#type: Type::Byte, legacy_type: 0, value };
    SetEntityData { runtime_entity_id: start.runtime_entity_id, metadata: vec![item], properties: EntityProperties { ints: vec![], floats: vec![] }, tick: 0 }
}

async fn place(d: &Driver, server: &FakeServer, bot: &str, cell: [i32; 3], name: &str) {
    server.set_blocks(&[cell], bds_block_id(name));
    let id = BotId(bot.to_owned());
    d.read(&format!("{name} in the bot's world"), |h| h.block_at(&id, BlockPos::from(cell)).filter(|b| b.name == name)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_food_and_the_cursor() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let _inventory = server.serve_inventory();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Miner", json!({ "physics": true, "autoEat": {} })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;
    let stone = [feet.x + 2, feet.y, feet.z];
    place(&d, &server, &bot, stone, "minecraft:stone").await;
    let at = |cell: [i32; 3]| json!({ "bot": bot, "x": cell[0], "y": cell[1], "z": cell[2] });

    assert_eq!(d.ok(2, "bot.equipBestTool", at(stone)).await, json!({ "slot": null }), "the bare hand is best");
    server.give(12, "minecraft:wooden_pickaxe", 1, 801);
    d.holding(&server, &bot, "minecraft:wooden_pickaxe", 1).await;
    let held = d.ok(3, "bot.equipBestTool", at(stone)).await;
    let slot = held["slot"].as_u64().unwrap_or_else(|| panic!("a hotbar slot: {held}"));
    d.hot_until(&bot, "the pickaxe held", |h| u64::from(h.held_slot) == slot).await;
    d.call(4, "bot.equipBestTool", at([feet.x + 400, feet.y, feet.z]));
    assert_eq!(code(&d.done(4).await.1), "NOT_POSSIBLE", "an unloaded cell");

    d.ok(5, "bot.setAutoEat", json!({ "bot": bot, "enabled": true, "below": 10, "prefer": "noWaste" })).await;
    d.ok(6, "bot.setAutoEat", json!({ "bot": bot, "enabled": false })).await;

    let planks = |r: &serde_json::Value| r["items"].as_array().unwrap().iter().any(|i| i["item"] == "minecraft:oak_planks");
    assert!(!planks(&d.ok(7, "bot.craftableItems", json!({ "bot": bot })).await), "no logs yet");
    server.give(0, "minecraft:oak_log", 1, 802);
    d.holding(&server, &bot, "minecraft:oak_log", 1).await;
    let craftable = d.ok(8, "bot.craftableItems", json!({ "bot": bot })).await;
    assert!(planks(&craftable), "{craftable}");

    let target = (stone[0] as f32, feet.y as f32 + 0.5, feet.z as f32 + 0.5);
    d.ok(9, "bot.lookAt", json!({ "bot": bot, "x": target.0, "y": target.1, "z": target.2 })).await;
    let (id, near) = (BotId(bot.clone()), BlockAtCursorParams { bot: BotId(bot.clone()), max_distance: None });
    let hit = d.read("the stone in sight", |h| h.block_at_cursor(&id, &near).ok()?.hit.filter(|hit| hit.position == BlockPos::from(stone))).await;
    assert_eq!((hit.block.name.as_str(), hit.face), ("minecraft:stone", Face::West), "{hit:?}");
    assert!(hit.distance > 1.0 && hit.distance < 2.0, "{hit:?}");
    let call = d.ok(10, "bot.blockAtCursor", json!({ "bot": bot })).await;
    assert_eq!(call["hit"]["position"], json!({ "x": stone[0], "y": stone[1], "z": stone[2] }), "{call}");
    assert_eq!(d.ok(11, "bot.blockAtCursor", json!({ "bot": bot, "maxDistance": 1 })).await, json!({ "hit": null }));
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn beds_and_signs() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Sleeper", json!({ "physics": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;
    let (bed, sign) = ([feet.x + 2, feet.y, feet.z], [feet.x - 2, feet.y, feet.z]);
    place(&d, &server, &bot, bed, "minecraft:bed").await;
    place(&d, &server, &bot, sign, "minecraft:standing_sign").await;
    let at = |cell: [i32; 3]| json!({ "bot": bot, "x": cell[0], "y": cell[1], "z": cell[2] });

    d.call(2, "bot.wake", json!({ "bot": bot }));
    assert_eq!(code(&d.done(2).await.1), "NOT_POSSIBLE", "not sleeping");
    d.call(3, "bot.sleep", at(sign));
    assert_eq!(code(&d.done(3).await.1), "NOT_POSSIBLE", "a sign is not a bed");

    d.call(4, "bot.sleep", at(bed));
    d.read("the click on the bed", |_| clicked(&server, bed).then_some(())).await;
    server.send(&player_flags(&server, SLEEPING));
    assert_eq!(d.ok_done(4).await, json!(null));
    d.read("StartSleeping, the bot confirming it lay down", |_| acted(&server, Action::StartSleeping).then_some(())).await;
    d.hot_until(&bot, "asleep", |h| h.sleeping).await;
    d.call(5, "bot.wake", json!({ "bot": bot }));
    d.read("leave bed", |_| acted(&server, Action::StopSleeping).then_some(())).await;
    server.send(&player_flags(&server, 0));
    assert_eq!(d.ok_done(5).await, json!(null));

    let mut write = at(sign);
    write["lines"] = json!(["hello", "world"]);
    d.call(6, "bot.writeSign", write);
    d.read("the click on the sign", |_| clicked(&server, sign).then_some(())).await;
    server.send(&OpenSign { position: BlockCoordinates { x: sign[0], y: sign[1], z: sign[2] }, is_front: true });
    assert_eq!(d.ok_done(6).await, json!(null));
    let text = |r: &acacia_testserver::Received| r.packet.id == BlockEntityData::ID && String::from_utf8_lossy(&r.packet.body).contains("hello\nworld");
    d.read("the sign's text at the server", |_| server.received().iter().any(text).then_some(())).await;
    let mut long = at(sign);
    long["lines"] = json!(["1", "2", "3", "4", "5"]);
    d.call(7, "bot.writeSign", long);
    assert_eq!(code(&d.done(7).await.1), "NOT_POSSIBLE", "five lines");
    d.host.shutdown().await;
}
