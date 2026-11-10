//! Crafting against the script's real recipes, with FakeServer accepting the item stack requests.

use acacia_bot::proto::packets::ItemStackRequest;
use acacia_bot::proto::types::{ItemStackRequestActionsItemTypeId as TypeId, WindowType};
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;

use super::common::{code, Driver};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recipes_and_crafts_in_the_grid_and_at_a_table() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let inventory = server.serve_inventory();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Crafter", json!({ "physics": true })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;

    let found = d.ok(2, "bot.recipesFor", json!({ "bot": bot, "item": "stick" })).await;
    let from_planks = |r: &&serde_json::Value| r["outputs"] == json!([{ "item": "minecraft:stick", "count": 4 }]);
    let sticks = found["recipes"].as_array().unwrap().iter().find(from_planks).unwrap_or_else(|| panic!("{found}"));
    assert_eq!((&sticks["station"], &sticks["fits2x2"]), (&json!("crafting_table"), &json!(true)), "{sticks}");
    assert_eq!(sticks["inputs"][0]["count"], 2, "two cells of one ingredient summed: {sticks}");

    d.call(3, "bot.craft", json!({ "bot": bot, "item": "oak_planks" }));
    assert_eq!(code(&d.done(3).await.1), "NOT_POSSIBLE", "no logs yet");
    server.give(0, "minecraft:oak_log", 3, 501);
    d.holding(&server, &bot, "minecraft:oak_log", 3).await;
    assert_eq!(d.ok(4, "bot.craft", json!({ "bot": bot, "item": "oak_planks", "count": 12 })).await, json!({ "crafted": 12 }));
    d.holding(&server, &bot, "minecraft:oak_planks", 12).await;
    d.holding(&server, &bot, "minecraft:oak_log", 0).await;
    let requests: Vec<ItemStackRequest> = server.received().iter().filter(|r| r.packet.id == ItemStackRequest::ID).filter_map(|r| r.packet.decode().ok()).collect();
    let first = &requests.first().expect("an item stack request").requests[0].actions[0];
    assert_eq!(first.type_id, TypeId::CraftRecipeAuto, "the recipe book's craft");

    assert_eq!(d.ok(5, "bot.craft", json!({ "bot": bot, "item": "stick" })).await, json!({ "crafted": 4 }));
    d.call(6, "bot.craft", json!({ "bot": bot, "item": "wooden_pickaxe" }));
    let (_, refused) = d.done(6).await;
    assert_eq!(code(&refused), "NOT_POSSIBLE", "a 3x3 recipe needs a table: {refused}");
    assert_eq!(d.ok(7, "bot.craft", json!({ "bot": bot, "item": "crafting_table" })).await, json!({ "crafted": 1 }));

    let table = [feet.x + 2, feet.y, feet.z];
    server.set_blocks(&[table], bds_block_id("minecraft:crafting_table"));
    let id = acacia_api::BotId(bot.clone());
    let at = acacia_api::BlockPos::from(table);
    d.read("the table in the bot's world", |h| h.block_at(&id, at).filter(|b| b.name == "minecraft:crafting_table")).await;
    inventory.add_container(&server, table, WindowType::Workbench, &[]);
    let params = json!({ "bot": bot, "item": "wooden_pickaxe", "table": { "x": table[0], "y": table[1], "z": table[2] } });
    assert_eq!(d.ok(8, "bot.craft", params).await, json!({ "crafted": 1 }));
    d.holding(&server, &bot, "minecraft:wooden_pickaxe", 1).await;
    d.holding(&server, &bot, "minecraft:oak_planks", 3).await;
    d.holding(&server, &bot, "minecraft:stick", 2).await;
    d.host.shutdown().await;
}
