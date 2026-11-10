//! The right-click and hand verbs, read back as the packets FakeServer got.

use std::time::{Duration, Instant};

use acacia_api::{BlockPos, BotId};
use acacia_bot::proto::packets::{Animate, InventoryTransaction, MobEquipment, Respawn, SetHealth};
use acacia_bot::proto::types::{
    TransactionTransactionData as Data, TransactionTransactionDataItemUseOnEntityActionType as OnEntity,
    TransactionUseItemActionType as UseItem,
};
use acacia_bot::proto::Packet;
use acacia_testserver::{bds_block_id, FakeServer, Script};
use serde_json::json;
use tokio::time::sleep;

use super::common::{code, Driver};

fn transactions(server: &FakeServer) -> Vec<Data> {
    let decoded = server.received().into_iter().filter(|r| r.packet.id == InventoryTransaction::ID);
    decoded.filter_map(|r| r.packet.decode::<InventoryTransaction>().ok()).map(|t| t.transaction.transaction_data).collect()
}

fn count<T: Packet>(server: &FakeServer) -> usize {
    server.received().iter().filter(|r| r.packet.id == T::ID).count()
}

/// Waits until the server has got an `InventoryTransaction` whose data matches.
async fn sent(server: &FakeServer, what: &str, want: impl Fn(&Data) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !transactions(server).iter().any(&want) {
        assert!(Instant::now() < deadline, "the server never got {what}: {:?}", transactions(server));
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hand_verbs_send_what_a_player_would() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let mut d = Driver::start();
    let bot = d.create_with(&server, 1, "Hands", json!({ "physics": true, "entities": true, "autoRespawn": false })).await;
    let feet = d.stand_on_floor(&server, &bot, 4).await;
    let b = json!({ "bot": bot });

    d.ok(2, "bot.setQuickBarSlot", json!({ "bot": bot, "slot": 3 })).await;
    d.hot_until(&bot, "slot 3 held", |h| h.held_slot == 3).await;
    let equipped = server.received().into_iter().filter_map(|r| r.packet.decode::<MobEquipment>().ok()).any(|m| m.selected_slot == 3);
    assert!(equipped, "MobEquipment names the slot");
    d.call(3, "bot.setQuickBarSlot", json!({ "bot": bot, "slot": 9 }));
    assert_eq!(code(&d.done(3).await.1), "NOT_POSSIBLE");

    d.ok(4, "bot.activateItem", b.clone()).await;
    sent(&server, "a ClickAir", |t| matches!(t, Data::ItemUse(u) if u.action_type == UseItem::ClickAir)).await;
    d.ok(5, "bot.deactivateItem", b.clone()).await;
    sent(&server, "a release", |t| matches!(t, Data::ItemRelease(_))).await;
    let swings = count::<Animate>(&server);
    d.ok(6, "bot.swingArm", b.clone()).await;
    d.read("the swing", |_| (count::<Animate>(&server) > swings).then_some(())).await;

    let cell = [feet.x + 2, feet.y, feet.z];
    server.set_blocks(&[cell], bds_block_id("minecraft:stone"));
    let id = BotId(bot.clone());
    d.read("the block in the bot's world", |h| h.block_at(&id, BlockPos::from(cell)).filter(|b| b.name == "minecraft:stone")).await;
    d.ok(7, "bot.activateBlock", json!({ "bot": bot, "x": cell[0], "y": cell[1], "z": cell[2] })).await;
    let at = |u: &acacia_bot::proto::types::TransactionUseItem| [u.block_position.x, u.block_position.y, u.block_position.z] == cell;
    sent(&server, "a ClickBlock on the cell", |t| matches!(t, Data::ItemUse(u) if u.action_type == UseItem::ClickBlock && at(u))).await;

    let (x, y, z) = (feet.x as f32 - 1.5, feet.y as f32, feet.z as f32 + 0.5);
    server.add_entity("minecraft:villager_v2", 950, 9500, [x, y, z]);
    d.tracked(&bot, 950).await;
    d.ok(8, "bot.activateEntity", json!({ "bot": bot, "entity": "950" })).await;
    let on = |t: &Data| matches!(t, Data::ItemUseOnEntity(e) if e.entity_runtime_id == 950 && e.action_type == OnEntity::Interact);
    sent(&server, "an Interact on the villager", on).await;

    d.call(9, "bot.respawn", b.clone());
    assert_eq!(code(&d.done(9).await.1), "NOT_POSSIBLE", "alive");
    server.send(&SetHealth { health: 0 });
    d.hot_until(&bot, "dead", |h| !h.alive).await;
    sleep(Duration::from_millis(2500)).await;
    assert_eq!(count::<Respawn>(&server), 0, "autoRespawn off: the bot waits on the death screen");
    d.ok(10, "bot.respawn", b).await;
    d.read("the respawn request", |_| (count::<Respawn>(&server) == 1).then_some(())).await;
    d.host.shutdown().await;
}
