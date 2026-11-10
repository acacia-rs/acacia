//! Container verbs: each opens a chest FakeServer serves, acts and closes it.

use acacia_api::{BlockPos, BotId};
use acacia_bot::proto::packets::ContainerClose;
use acacia_bot::proto::types::WindowType;
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;

use super::common::{code, Driver};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contents_withdraw_and_deposit() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let inventory = server.serve_inventory();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Hoarder", json!({ "physics": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;
    let chest = [feet.x + 2, feet.y, feet.z];
    server.set_blocks(&[chest], bds_block_id("minecraft:chest"));
    let id = BotId(bot.clone());
    d.read("the chest in the bot's world", |h| h.block_at(&id, BlockPos::from(chest)).filter(|b| b.name == "minecraft:chest")).await;
    inventory.add_container(&server, chest, WindowType::Container, &[(0, "minecraft:cobblestone", 20), (3, "minecraft:oak_log", 5)]);
    let at = |extra: serde_json::Value| {
        let mut params = json!({ "bot": bot, "x": chest[0], "y": chest[1], "z": chest[2] });
        params.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        params
    };

    let contents = d.ok(2, "bot.containerContents", at(json!({}))).await;
    let want = json!({ "items": [{ "slot": 0, "item": "minecraft:cobblestone", "count": 20 }, { "slot": 3, "item": "minecraft:oak_log", "count": 5 }] });
    assert_eq!(contents, want);
    let closes = || server.received().iter().filter(|r| r.packet.id == ContainerClose::ID).count();
    assert_eq!(closes(), 1, "closed after reading");

    assert_eq!(d.ok(3, "bot.withdraw", at(json!({ "item": "oak_log", "count": 3 }))).await, json!({ "moved": 3 }));
    d.holding(&server, &bot, "minecraft:oak_log", 3).await;

    server.give(2, "minecraft:cobblestone", 10, 601);
    d.holding(&server, &bot, "minecraft:cobblestone", 10).await;
    assert_eq!(d.ok(4, "bot.deposit", at(json!({ "item": "cobblestone" }))).await, json!({ "moved": 10 }));
    d.holding(&server, &bot, "minecraft:cobblestone", 0).await;

    d.call(5, "bot.deposit", at(json!({ "item": "diamond" })));
    let (_, refused) = d.done(5).await;
    assert_eq!(code(&refused), "NOT_POSSIBLE", "{refused}");
    assert_eq!(closes(), 4, "closed after a refusal too");
    d.host.shutdown().await;
}
