//! Riding, item moves and the workstation verbs' refusals, with FakeServer seating the bot and accepting item
//! requests.

use acacia_api::BotId;
use acacia_bot::proto::packets::{InventoryTransaction, SetEntityLink, StartGame};
use acacia_bot::proto::types::{Link, TransactionTransactionData as Data, TransactionTransactionDataItemUseOnEntityActionType as OnEntity};
use acacia_bot::proto::Packet;
use acacia_testserver::{FakeServer, Script};
use serde_json::{json, Value};

use super::common::{code, Driver};

/// acacia-bot's link type for the controlling seat.
const LINK_RIDER: u8 = 1;

fn own_unique_id(server: &FakeServer) -> i64 {
    let start: StartGame = server.sent().iter().find(|r| r.packet.id == StartGame::ID).and_then(|r| r.packet.decode().ok()).unwrap();
    start.entity_id
}

fn clicked_entity(server: &FakeServer, id: u64) -> bool {
    let decoded = server.received().into_iter().filter(|r| r.packet.id == InventoryTransaction::ID);
    decoded.filter_map(|r| r.packet.decode::<InventoryTransaction>().ok()).any(|t| {
        matches!(t.transaction.transaction_data, Data::ItemUseOnEntity(e) if e.entity_runtime_id == id && e.action_type == OnEntity::Interact)
    })
}

fn slot(window: i32, slot: u8) -> Value {
    json!({ "window": window, "slot": slot })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn item_moves_and_riding() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let _inventory = server.serve_inventory();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Rider", json!({ "physics": true, "entities": true })).await;
    let id = BotId(bot.clone());
    let feet = d.stand_on_floor(&server, &bot, 4).await;

    server.give(2, "minecraft:stone", 16, 77);
    server.give(3, "minecraft:dirt", 4, 78);
    d.holding(&server, &bot, "minecraft:dirt", 4).await;
    let has = |slot: u8, name: &str, count: u16| {
        let (net, id) = (i32::from(server.item_id(name).unwrap()), id.clone());
        move |h: &acacia_host::Host| h.cold(&id).filter(|c| c.inventory.iter().any(|s| (s.window, s.slot, s.network_id, s.count) == (0, slot, net, count)))
    };
    d.ok(2, "bot.moveItem", json!({ "bot": bot, "from": slot(0, 2), "to": slot(0, 10), "count": 5 })).await;
    d.read("5 stone moved to slot 10", has(10, "minecraft:stone", 5)).await;
    d.read("11 stone left in slot 2", has(2, "minecraft:stone", 11)).await;
    d.ok(3, "bot.swapItems", json!({ "bot": bot, "a": "dirt", "b": slot(0, 10) })).await;
    d.read("the dirt in slot 10", has(10, "minecraft:dirt", 4)).await;
    d.ok(4, "bot.moveItem", json!({ "bot": bot, "from": slot(0, 2), "to": slot(0, 20) })).await;
    d.read("the whole stack moved", has(20, "minecraft:stone", 11)).await;
    for (call, params) in [
        (5, json!({ "bot": bot, "from": slot(0, 2), "to": slot(0, 21) })),
        (6, json!({ "bot": bot, "from": slot(7, 0), "to": slot(0, 21) })),
        (7, json!({ "bot": bot, "from": "diamond", "to": slot(0, 21) })),
    ] {
        d.call(call, "bot.moveItem", params.clone());
        assert_eq!(code(&d.done(call).await.1), "NOT_POSSIBLE", "{params}");
    }

    d.call(8, "bot.dismount", json!({ "bot": bot }));
    assert_eq!(code(&d.done(8).await.1), "NOT_POSSIBLE", "not riding");
    server.add_entity("minecraft:boat", 960, 9600, [feet.x as f32 + 1.5, feet.y as f32, feet.z as f32 + 0.5]);
    d.tracked(&bot, 960).await;
    d.call(9, "bot.mount", json!({ "bot": bot, "entity": "960" }));
    d.read("the click on the boat", |_| clicked_entity(&server, 960).then_some(())).await;
    let link = Link { ridden_entity_id: 9600, rider_entity_id: own_unique_id(&server), r#type: LINK_RIDER, immediate: false, rider_initiated: true, angular_velocity: 0.0 };
    server.send(&SetEntityLink { link });
    let vehicle = d.ok_done(9).await;
    assert_eq!(vehicle, json!({ "uniqueId": "9600", "id": "960", "kind": "minecraft:boat", "driver": true }));
    d.hot_until(&bot, "riding", |h| h.riding).await;
    d.ok(10, "bot.dismount", json!({ "bot": bot })).await;
    d.hot_until(&bot, "off the boat", |h| !h.riding).await;
    d.host.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workstations_refuse_what_cannot_work() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Smith", json!({ "physics": true, "entities": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;
    server.give(0, "minecraft:iron_sword", 1, 90);
    d.holding(&server, &bot, "minecraft:iron_sword", 1).await;
    let at = |extra: Value| {
        let mut p = json!({ "bot": bot, "x": feet.x + 1, "y": feet.y, "z": feet.z });
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let cases = [
        ("bot.anvil", at(json!({ "input": "iron_sword" })), "NOT_POSSIBLE"),
        ("bot.enchant", at(json!({ "item": "diamond_sword", "option": 0 })), "NOT_POSSIBLE"),
        ("bot.grindstone", at(json!({ "input": slot(120, 0) })), "NOT_POSSIBLE"),
        ("bot.smith", at(json!({ "base": "iron_sword", "addition": "netherite_ingot", "template": slot(0, 5) })), "NOT_POSSIBLE"),
        ("bot.stonecut", at(json!({ "item": "no_such_item" })), "NOT_POSSIBLE"),
        ("bot.loom", at(json!({ "banner": "white_banner", "dye": "red_dye", "pattern": "bo" })), "NOT_POSSIBLE"),
        ("bot.cartography", at(json!({ "map": slot(0, 0) })), "NOT_POSSIBLE"),
        ("bot.beacon", at(json!({ "primary": "speed", "payment": slot(0, 0) })), "NOT_POSSIBLE"),
        ("bot.brew", at(json!({ "bottles": [], "ingredient": "nether_wart" })), "NOT_POSSIBLE"),
        ("bot.brew", at(json!({ "bottles": ["potion"], "ingredient": "nether_wart" })), "NOT_POSSIBLE"),
        ("bot.trade", json!({ "bot": bot, "entity": "999", "offer": 0 }), "NOT_POSSIBLE"),
    ];
    for (call, (method, params, want)) in (2..).zip(cases) {
        d.call(call, method, params.clone());
        assert_eq!(code(&d.done(call).await.1), want, "{method} {params}");
    }
    let unknown = json!({ "call": 99, "method": "bot.beacon", "params": at(json!({ "primary": "flight", "payment": slot(0, 0) })) });
    let refused = d.host.call(&unknown.to_string()).expect_err("an unknown power");
    assert!(refused.to_string().contains("BadParams"), "{refused}");
    d.host.shutdown().await;
}
