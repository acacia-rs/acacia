//! Entities and other players: events, the published list, and walking to one.

use acacia_api::BotId;
use acacia_testserver::{FakeServer, Script};
use serde_json::json;

use super::common::Driver;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn entities_come_and_go_and_a_bot_walks_to_a_player() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "EntityTester", json!({ "physics": true, "entities": true, "events": ["entities"] })).await;
    let id = BotId(bot.clone());
    let feet = d.stand_on_floor(&server, &bot, 8).await;
    let (x, y, z) = (feet.x as f32 + 0.5, feet.y as f32, feet.z as f32 + 0.5);

    server.add_entity("minecraft:zombie", 900, 9000, [x - 3.0, y, z]);
    let zombie = d.event(&bot, "zombie spawn", |e| e["type"] == "entitySpawn" && e["id"] == "900").await;
    assert_eq!((zombie["kind"].as_str(), zombie["uniqueId"].as_str()), (Some("minecraft:zombie"), Some("9000")), "{zombie}");
    assert_eq!(zombie["position"]["y"], y as f64, "feet: {zombie}");
    server.add_player("Steve", 901, 9001, [x + 5.0, y, z]);
    let steve = d.event(&bot, "Steve spawn", |e| e["type"] == "entitySpawn" && e["id"] == "901").await;
    assert_eq!(steve["username"], "Steve");
    assert_eq!(steve["position"]["y"], y as f64, "a player's position is its feet too: {steve}");

    let listed = |l: &acacia_api::EntityList, want: u64| l.entities.iter().any(|e| e.id.0 == want);
    let list = d.read("both in the entity list", |h| h.entities(&id).filter(|l| listed(l, 900) && listed(l, 901))).await;
    assert_eq!(d.host.entities_version(&id), Some(list.version));
    server.remove_entity(9000);
    d.event(&bot, "zombie gone", |e| e["type"] == "entityGone" && e["id"] == "900").await;
    let snapshot = d.ok(2, "bot.snapshot", json!({ "bot": bot })).await;
    let ids: Vec<_> = snapshot["entities"].as_array().unwrap().iter().map(|e| e["id"].clone()).collect();
    assert!(ids.contains(&json!("901")) && !ids.contains(&json!("900")), "{snapshot}");

    let start = d.hot(&bot).position;
    let goal = json!({ "type": "near", "x": feet.x + 5, "y": feet.y, "z": feet.z, "range": 2.0 });
    d.ok(3, "bot.goto", json!({ "bot": bot, "goal": goal })).await;
    let end = d.hot(&bot).position;
    let to_steve = |p: acacia_api::Vec3| ((p.x - (x + 5.0)).powi(2) + (p.z - z).powi(2)).sqrt();
    assert!(to_steve(end) <= 2.5 && to_steve(end) < to_steve(start) - 2.0, "walked from {start:?} to {end:?}");
    d.host.shutdown().await;
}
