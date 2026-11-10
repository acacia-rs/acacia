//! Inventory and blocks: the item table, equip, findBlocks, placeBlock.

use acacia_api::{BlockPos, BotId, FindBlocksParams};
use acacia_bot::proto::packets::{InventoryTransaction, MobEquipment};
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;

use super::common::{code, received, Driver};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn equip_find_and_place() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Builder", json!({ "physics": true })).await;
    let id = BotId(bot.clone());
    let feet = d.stand_on_floor(&server, &bot, 4).await;

    let stone = server.item_id("minecraft:stone").expect("the script's item table has stone");
    let table = d.host.item_table(&id).unwrap();
    assert!(table.items.iter().any(|i| i.id == i32::from(stone) && i.name == "minecraft:stone"), "{} items", table.items.len());
    server.give(2, "minecraft:stone", 16, 77);
    let cold = d.read("the stone in the inventory", |h| h.cold(&id).filter(|c| c.inventory.iter().any(|s| s.network_id == i32::from(stone)))).await;
    let stack = cold.inventory.iter().find(|s| s.network_id == i32::from(stone)).unwrap();
    assert_eq!((stack.window, stack.slot, stack.count), (0, 2, 16));

    d.call(2, "bot.equip", json!({ "bot": bot, "item": "diamond" }));
    assert_eq!(code(&d.done(2).await.1), "NOT_POSSIBLE");
    d.ok(3, "bot.equip", json!({ "bot": bot, "item": "stone" })).await;
    d.hot_until(&bot, "slot 2 held", |h| h.held_slot == 2).await;
    received(&server, MobEquipment::ID).await;

    let found = d.ok(4, "bot.findBlocks", json!({ "bot": bot, "matching": ["stone"], "maxDistance": 4, "count": 3 })).await;
    let blocks: Vec<BlockPos> = serde_json::from_value(found["blocks"].clone()).unwrap();
    let under = BlockPos { x: feet.x, y: feet.y - 1, z: feet.z };
    assert_eq!((blocks.len(), blocks[0]), (3, under), "{found}");
    let params = FindBlocksParams { bot: id.clone(), matching: vec!["minecraft:stone".into()], max_distance: Some(4), count: Some(3) };
    assert_eq!(d.host.find_blocks(&id, &params).unwrap().blocks, blocks, "the sync read agrees");
    d.call(5, "bot.findBlocks", json!({ "bot": bot, "matching": ["no_such_block"] }));
    assert_eq!(code(&d.done(5).await.1), "BAD_PARAMS");

    let against = [feet.x + 1, feet.y - 1, feet.z];
    d.call(6, "bot.placeBlock", json!({ "bot": bot, "x": against[0], "y": against[1], "z": against[2], "face": "up" }));
    received(&server, InventoryTransaction::ID).await;
    server.set_blocks(&[[against[0], feet.y, against[2]]], bds_block_id("minecraft:stone"));
    let (_, placed) = d.done(6).await;
    assert!(placed.get("ok").is_some(), "the placement confirmed once the server showed it: {placed}");
    d.host.shutdown().await;
}
