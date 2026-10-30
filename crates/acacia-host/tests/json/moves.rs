//! Movement and combat verbs: lookAt, held keys, attack, follow.

use acacia_api::Vec3;
use acacia_bot::proto::packets::InventoryTransaction;
use acacia_bot::proto::Packet;
use acacia_testserver::{FakeServer, Script};
use serde_json::json;

use super::common::{code, received, Driver};

fn flat(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn look_keys_attack_and_follow() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Mover", json!({ "physics": true, "entities": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 10).await;
    let (x, y, z) = (feet.x as f32 + 0.5, feet.y as f32, feet.z as f32 + 0.5);

    d.ok(2, "bot.lookAt", json!({ "bot": bot, "x": x + 10.0, "y": y + 1.62, "z": z })).await;
    let yaw = d.hot(&bot).yaw;
    assert!((yaw + 90.0).abs() < 1.0, "facing +x is yaw -90, got {yaw}");

    let start = d.hot(&bot).position;
    d.ok(3, "bot.setControlState", json!({ "bot": bot, "control": "forward", "state": true })).await;
    d.hot_until(&bot, "walking forward", |h| h.position.x > start.x + 2.0).await;
    d.ok(4, "bot.clearControlStates", json!({ "bot": bot })).await;
    d.hot_until(&bot, "stopping", |h| h.velocity.x.abs() < 0.01).await;
    let stopped = d.hot(&bot).position;
    assert!((stopped.z - start.z).abs() < 0.1, "forward is +x after the look: {start:?} -> {stopped:?}");

    server.add_entity("minecraft:zombie", 900, 9000, [stopped.x + 2.0, y, stopped.z]);
    d.tracked(&bot, 900).await;
    d.ok(5, "bot.attack", json!({ "bot": bot, "entity": "900" })).await;
    received(&server, InventoryTransaction::ID).await;
    d.call(6, "bot.attack", json!({ "bot": bot, "entity": "12345" }));
    assert_eq!(code(&d.done(6).await.1), "NOT_POSSIBLE");
    d.call(7, "bot.follow", json!({ "bot": bot, "entity": "12345" }));
    assert_eq!(code(&d.done(7).await.1), "NOT_POSSIBLE");

    let steve = Vec3 { x: stopped.x - 6.0, y, z: stopped.z };
    server.add_player("Steve", 901, 9001, [steve.x, steve.y, steve.z]);
    d.tracked(&bot, 901).await;
    d.call(8, "bot.follow", json!({ "bot": bot, "entity": "901", "range": 2.0 }));
    d.hot_until(&bot, "close to Steve", |h| flat(h.position, steve) < 3.0).await;
    assert!(!d.seen.iter().any(|r| r["type"] == "done" && r["call"] == 8), "follow keeps running");
    d.ok(9, "cancel", json!({ "bot": bot, "call": 8 })).await;
    assert_eq!(code(&d.done(8).await.1), "CANCELLED");
    d.host.shutdown().await;
}
