//! Smelting, eating and tossing, with FakeServer playing the furnace and the hunger bar by hand.

use acacia_api::{BlockPos, BotId};
use acacia_bot::proto::packets::{EntityEvent, ItemStackRequest};
use acacia_bot::proto::types::{ItemStackRequestActionsItemTypeId as TypeId, WindowType};
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;

use super::common::{code, received, Driver};

fn requests(server: &FakeServer) -> Vec<ItemStackRequest> {
    server.received().iter().filter(|r| r.packet.id == ItemStackRequest::ID).filter_map(|r| r.packet.decode().ok()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn smelt_consume_and_toss() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let inventory = server.serve_inventory();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Cook", json!({ "physics": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;

    let furnace = [feet.x + 2, feet.y, feet.z];
    server.set_blocks(&[furnace], bds_block_id("minecraft:furnace"));
    let id = BotId(bot.clone());
    d.read("the furnace in the bot's world", |h| h.block_at(&id, BlockPos::from(furnace)).filter(|b| b.name == "minecraft:furnace")).await;
    inventory.add_container(&server, furnace, WindowType::Furnace, &[]);
    server.give(0, "minecraft:raw_iron", 2, 701);
    server.give(1, "minecraft:coal", 1, 702);
    d.holding(&server, &bot, "minecraft:coal", 1).await;
    let before = requests(&server).len();
    let params = json!({ "bot": bot, "x": furnace[0], "y": furnace[1], "z": furnace[2], "item": "raw_iron", "count": 2, "fuel": "coal" });
    d.call(2, "bot.smelt", params);
    let window = d.hot_until(&bot, "the furnace open", |h| h.open_window.is_some()).await.open_window.unwrap();
    d.read("the input and fuel put in", |_| (requests(&server).len() >= before + 2).then_some(())).await;
    server.set_slot(window.into(), 0, None, 0);
    server.set_slot(window.into(), 2, Some(("minecraft:iron_ingot", 2)), 703);
    assert_eq!(d.ok_done(2).await, json!({ "smelted": 2 }));
    d.holding(&server, &bot, "minecraft:iron_ingot", 2).await;
    d.holding(&server, &bot, "minecraft:raw_iron", 0).await;

    d.call(3, "bot.consume", json!({ "bot": bot }));
    assert_eq!(code(&d.done(3).await.1), "NOT_POSSIBLE", "the hand is empty");
    server.give(3, "minecraft:golden_apple", 2, 704);
    d.holding(&server, &bot, "minecraft:golden_apple", 2).await;
    d.call(4, "bot.consume", json!({ "bot": bot, "item": "golden_apple" }));
    received(&server, EntityEvent::ID).await;
    server.give(3, "minecraft:golden_apple", 1, 705);
    assert_eq!(d.ok_done(4).await, json!(null));
    d.hot_until(&bot, "the apple in hand", |h| h.held_slot == 3).await;

    server.give(5, "minecraft:cobblestone", 5, 706);
    d.holding(&server, &bot, "minecraft:cobblestone", 5).await;
    let toward = json!({ "x": feet.x as f32 + 3.0, "y": feet.y as f32 + 1.6, "z": feet.z as f32 });
    assert_eq!(d.ok(5, "bot.toss", json!({ "bot": bot, "item": "cobblestone", "count": 3, "toward": toward })).await, json!({ "tossed": 3 }));
    d.holding(&server, &bot, "minecraft:cobblestone", 2).await;
    let last = requests(&server).pop().expect("the drop's request");
    assert_eq!(last.requests[0].actions[0].type_id, TypeId::Drop);
    d.call(6, "bot.toss", json!({ "bot": bot, "item": "diamond" }));
    assert_eq!(code(&d.done(6).await.1), "NOT_POSSIBLE");
    d.host.shutdown().await;
}
