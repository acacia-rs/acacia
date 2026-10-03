use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::packets::{Animate, InventoryTransaction, MobEquipment, PlayerAuthInput};
use acacia_client::proto::types::{
    Action, ItemV4Extra, TransactionTransactionData as Data, TransactionTransactionDataItemUseOnEntityActionType as EntityAction,
    TransactionTransactionType, TransactionUseItemActionType, TransactionUseItemClientPrediction as Prediction,
    TransactionUseItemTriggerType as Trigger, Vec3f, WindowID,
};
use acacia_client::proto::{encode_packet, Packet, RawPacket};
use bytes::BytesMut;

use super::breaking::enchantment_level;
use super::geometry::{block_distance, entity_aim, ray_box};
use super::wire::{self, Hand, SwingSource};
use super::{facing_face, to_wire, Face};
use crate::state::{Entity, ItemStack, PLAYER_KIND};

fn round_trip<T: Packet + PartialEq + std::fmt::Debug>(packet: &T) -> T {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    let raw = RawPacket::parse(buf.freeze()).unwrap();
    assert_eq!(raw.id, T::ID);
    let decoded: T = raw.decode().unwrap();
    assert_eq!(&decoded, packet);
    decoded
}

fn enchanted_pickaxe() -> ItemStack {
    let ench = |id: i16, lvl: i16| Value::Compound(vec![("id".into(), Value::Short(id)), ("lvl".into(), Value::Short(lvl))]);
    let list = List { tag: 10, items: vec![ench(17, 3), ench(15, 5)] };
    ItemStack {
        network_id: 330,
        count: 1,
        stack_network_id: Some(12),
        nbt: Some(Nbt { name: String::new(), value: Value::Compound(vec![("ench".into(), Value::List(list))]) }),
        ..ItemStack::default()
    }
}

fn hand() -> Hand {
    Hand { slot: 2, item: to_wire(&enchanted_pickaxe()), eye: [1.5, 65.62, -2.5] }
}

#[test]
fn wire_items() {
    let air = to_wire(&ItemStack::default());
    assert_eq!((air.network_id, air.count, air.has_stack_id, air.extra), (0, 0, false, ItemV4Extra::Default(None)));

    let pick = to_wire(&enchanted_pickaxe());
    assert_eq!((pick.network_id, pick.count, pick.stack_id), (330, 1, Some(12)));
    let ItemV4Extra::Default(Some(extra)) = &pick.extra else { panic!("user data missing: {:?}", pick.extra) };
    assert_eq!(extra.nbt.as_ref().map(|n| n.version), Some(1));

    // A present item without NBT still carries the (empty) user-data blob.
    let plain = to_wire(&ItemStack { network_id: 1, count: 64, ..ItemStack::default() });
    assert!(matches!(plain.extra, ItemV4Extra::Default(Some(_))));
    assert!(!plain.has_stack_id);

    let shield = to_wire(&ItemStack { network_id: acacia_client::proto::manual::shield_item_id(), count: 1, ..ItemStack::default() });
    assert!(matches!(shield.extra, ItemV4Extra::ShieldItemID(Some(_))));
    let equip = round_trip(&MobEquipment { item: shield, ..wire::mob_equipment(1, 0, &ItemStack::default()) });
    assert!(matches!(equip.item.extra, ItemV4Extra::ShieldItemID(Some(_))));
}

#[test]
fn enchantments_from_nbt() {
    assert_eq!(enchantment_level(&enchanted_pickaxe(), 15), 5);
    assert_eq!(enchantment_level(&enchanted_pickaxe(), 8), 0);
    assert_eq!(enchantment_level(&ItemStack::default(), 15), 0);
}

#[test]
fn hotbar_and_swing_packets() {
    let p = round_trip(&wire::mob_equipment(7, 3, &enchanted_pickaxe()));
    assert_eq!((p.runtime_entity_id, p.slot, p.selected_slot, p.window_id), (7, 3, 3, WindowID::Inventory));
    assert_eq!(p.item.network_id, 330);

    let s = round_trip(&wire::swing(7, Some(SwingSource::Attack)));
    assert_eq!((s.runtime_entity_id, s.swing_source.as_deref()), (7, Some("attack")));
    let plain: Animate = round_trip(&wire::swing(7, None));
    assert!(!plain.has_swing_source);
}

#[test]
fn click_block_transaction() {
    let t = round_trip(&wire::click_block(hand(), [10, 64, -3], Face::Up, Face::Up.click_offset(), 1234));
    assert_eq!(t.transaction.transaction_type, TransactionTransactionType::ItemUse);
    assert_eq!(t.transaction.legacy.legacy_request_id, 0);
    assert!(t.transaction.actions.is_empty());
    let Data::ItemUse(u) = t.transaction.transaction_data else { panic!() };
    assert_eq!(u.action_type, TransactionUseItemActionType::ClickBlock);
    assert_eq!((u.block_position.x, u.block_position.y, u.block_position.z), (10, 64, -3));
    assert_eq!((u.face, u.hotbar_slot, u.block_runtime_id), (1, 2, 1234));
    assert_eq!(u.click_pos, Vec3f { x: 0.5, y: 1.0, z: 0.5 });
    assert_eq!(u.player_pos, Vec3f { x: 1.5, y: 65.62, z: -2.5 });
    assert_eq!(u.held_item.network_id, 330);
}

#[test]
fn click_air_and_release() {
    let InventoryTransaction { transaction } = round_trip(&wire::click_air(hand()));
    let Data::ItemUse(u) = transaction.transaction_data else { panic!() };
    assert_eq!(u.action_type, TransactionUseItemActionType::ClickAir);
    assert_eq!((u.block_position.x, u.block_position.y, u.block_position.z, u.face), (0, 0, 0, 255));
    assert_eq!((u.trigger_type, u.client_prediction), (Trigger::UnknownValue, Prediction::Failure));

    let InventoryTransaction { transaction } = round_trip(&wire::release(hand()));
    assert_eq!(transaction.transaction_type, TransactionTransactionType::ItemRelease);
    let Data::ItemRelease(r) = transaction.transaction_data else { panic!() };
    assert_eq!((r.hotbar_slot, r.head_pos.y), (2, 65.62));
}

#[test]
fn entity_transactions() {
    for (attack, action) in [(true, EntityAction::Attack), (false, EntityAction::Interact)] {
        let InventoryTransaction { transaction } = round_trip(&wire::use_on_entity(hand(), 99, attack, [1.0, 2.0, 3.0]));
        assert_eq!(transaction.transaction_type, TransactionTransactionType::ItemUseOnEntity);
        let Data::ItemUseOnEntity(e) = transaction.transaction_data else { panic!() };
        assert_eq!((e.entity_runtime_id, e.action_type, e.hotbar_slot), (99, action, 2));
        assert_eq!(e.click_pos, Vec3f { x: 1.0, y: 2.0, z: 3.0 });
    }
}

#[test]
fn block_actions_ride_in_auth_input() {
    let actions = vec![
        wire::block_action(Action::ContinueBreak, [1, -60, 2], Face::North),
        wire::block_action(Action::PredictBreak, [1, -60, 2], Face::North),
    ];
    let mut input = PlayerAuthInput { block_action: None, ..idle_input() };
    crate::movement::attach_actions(&mut input, actions.clone());
    let decoded = round_trip(&input);
    assert_eq!(decoded.block_action, Some(actions));
    assert_eq!(decoded.block_action.unwrap()[1].face, 2);
}

fn idle_input() -> PlayerAuthInput {
    let raw = hex_fixture("player_auth_input");
    raw.decode().unwrap()
}

fn hex_fixture(name: &str) -> RawPacket {
    let path = format!("{}/../acacia-proto/tests/fixtures/packets/{name}.hex", env!("CARGO_MANIFEST_DIR"));
    let hex = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let line = hex.lines().find(|l| !l.is_empty()).unwrap();
    let bytes: Vec<u8> = (0..line.len()).step_by(2).map(|i| u8::from_str_radix(&line[i..i + 2], 16).unwrap()).collect();
    RawPacket::parse(bytes.into()).unwrap()
}

#[test]
fn faces_and_offsets() {
    assert_eq!(Face::East.adjacent([0, 64, 0]), [1, 64, 0]);
    assert_eq!(Face::Down.click_offset(), [0.5, 0.0, 0.5]);
    assert_eq!(Face::West.click_offset(), [0.0, 0.5, 0.5]);
    // Eye above and to the east: steep view enters through the top, flat view through the side.
    assert_eq!(facing_face([0.5, 70.0, 0.5], [0, 64, 0]), Face::Up);
    assert_eq!(facing_face([3.5, 64.6, 0.5], [0, 64, 0]), Face::East);
    assert_eq!(facing_face([0.5, 64.5, -3.0], [0, 64, 0]), Face::North);
    assert_eq!(facing_face([0.5, 60.0, 0.5], [0, 64, 0]), Face::Down);
}

fn player_at(feet: [f32; 3]) -> Entity {
    Entity {
        runtime_id: 5,
        unique_id: -5,
        kind: PLAYER_KIND.into(),
        username: None,
        position: Vec3f { x: feet[0], y: feet[1] + 1.62, z: feet[2] },
        yaw: 0.0,
        pitch: 0.0,
        head_yaw: 0.0,
        velocity: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
        on_ground: true,
        movement: None,
    }
}

#[test]
fn reach() {
    let eye = [0.0, 65.62, 0.0];
    // Hitbox face 0.3 from its centre: 3.2 - 0.3 = 2.9 along the flat ray.
    let (aim, hit, d) = entity_aim(eye, &player_at([0.0, 64.72, 3.2]));
    assert_eq!(aim, [0.0, 65.62, 3.2]);
    assert!((d - 2.9).abs() < 1e-4, "{d}");
    assert!((hit[2] - 2.9).abs() < 1e-4);
    let (_, _, d) = entity_aim(eye, &player_at([0.0, 64.72, 3.4]));
    assert!(d > super::ENTITY_REACH);
    let (_, _, inside) = entity_aim(eye, &player_at([0.1, 64.0, 0.0]));
    assert_eq!(inside, 0.0);

    // A boat on the ground at y 64 (wire y 64.375) is aimed at low; the ray enters its top face.
    let boat = Entity { kind: "minecraft:boat".into(), position: Vec3f { x: 0.0, y: 64.375, z: 3.2 }, ..player_at([0.0; 3]) };
    let (aim, hit, d) = entity_aim(eye, &boat);
    assert!((aim[1] - 64.2275).abs() < 1e-4 && (hit[1] - 64.455).abs() < 1e-4, "{aim:?} {hit:?}");
    assert!((d - 2.92).abs() < 0.01, "{d}");

    assert!((block_distance([0.5, 64.5, 0.5], [0, 64, 5]) - 4.5).abs() < 1e-4);
    assert!((block_distance([0.5, 66.0, 0.0], [0, 62, 4]) - 5.0).abs() < 1e-4);
    assert_eq!(block_distance([0.5, 64.5, 0.5], [0, 64, 0]), 0.0);
    assert_eq!(ray_box([0.0; 3], [1.0, 0.0, 0.0], [2.0, -1.0, -1.0], [3.0, 1.0, 1.0]), Some((2.0, 0)));
    assert_eq!(ray_box([0.0; 3], [-1.0, 0.0, 0.0], [2.0, -1.0, -1.0], [3.0, 1.0, 1.0]), None);
}
