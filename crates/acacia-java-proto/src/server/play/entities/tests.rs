use super::effects::*;
use super::movement::*;
use super::player_info::*;
use super::serverbound::*;
use super::spawn::*;
use super::status::*;
use super::{OptEntityId, Optional, pack_degrees, unpack_degrees};
use crate::actions::Hand;
use crate::entity::{LpVec3, PositionPath};
use crate::item::ItemStack;
use crate::packets::PositionMoveRotation;
use crate::server::login::ProfileProperty;
use crate::server::round_trip;
use crate::text::Component;
use crate::types::{AttributeModifierOperation, GameMode, Vec3};
use crate::{VarInt, Wire};

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

#[test]
fn add_entity_is_id_uuid_type_position_movement_angles_data() {
    let packet = AddEntity {
        id: VarInt(300),
        uuid: 0x0123_4567_89AB_CDEF_0011_2233_4455_6677,
        entity_type: VarInt(5),
        x: 1.0,
        y: 2.0,
        z: 3.0,
        movement: LpVec3([0.0; 3]),
        x_rot: 1,
        y_rot: 2,
        y_head_rot: 3,
        data: VarInt(4),
    };
    let bytes = round_trip(packet);
    let uuid = 0x0123_4567_89AB_CDEF_0011_2233_4455_6677u128.to_be_bytes();
    let doubles = cat(&[&1.0f64.to_be_bytes(), &2.0f64.to_be_bytes(), &3.0f64.to_be_bytes()]);
    assert_eq!(bytes, cat(&[&[0xAC, 0x02], &uuid, &[5], &doubles, &[0], &[1, 2, 3, 4]]));
}

#[test]
fn remove_entities_is_a_varint_list() {
    assert_eq!(round_trip(RemoveEntities { ids: vec![VarInt(1), VarInt(200)] }), [2, 1, 0xC8, 0x01]);
}

#[test]
fn degrees_pack_into_256ths() {
    assert_eq!((pack_degrees(90.0), pack_degrees(-90.0), pack_degrees(360.0)), (64, 192, 0));
    assert_eq!(unpack_degrees(64), 90.0);
    assert_eq!(unpack_degrees(192), -90.0);
}

#[test]
fn relative_moves_pack_ground_and_step_count_into_properties() {
    let linear = MoveEntityPos { entity: VarInt(7), on_ground: true, delta: VecDelta::Linear([1, -1, 0]) };
    assert_eq!(round_trip(linear), [7, 1, 0, 1, 0xFF, 0xFF, 0, 0]);
    let stepped = MoveEntityPosRot { entity: VarInt(7), on_ground: false, delta: VecDelta::Stepped(vec![(3, [1, 0, 0]), (2, [0, 2, 0])]), y_rot: 9, x_rot: 8 };
    assert_eq!(round_trip(stepped), [7, 4, 3, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 9, 8]);
    assert_eq!(round_trip(MoveEntityRot { entity: VarInt(7), on_ground: true, y_rot: 9, x_rot: 8 }), [7, 1, 9, 8]);
}

#[test]
fn relative_moves_agree_with_the_client_side_reader() {
    let mut stepped = vec![7, 0b101];
    for (ticks, dx) in [(3u8, 4096i16), (2, 2048)] {
        stepped.push(ticks);
        dx.write(&mut stepped);
        0i16.write(&mut stepped);
        0i16.write(&mut stepped);
    }
    let (on_ground, path) = crate::entity::read_move(&mut &stepped[1..], [1.0, 2.0, 3.0]).unwrap();
    assert!(on_ground);
    assert_eq!(path.end(), [2.5, 2.0, 3.0]);
    let decoded = MoveEntityPos::read(&mut &stepped[..]).unwrap();
    assert_eq!(decoded.delta, VecDelta::Stepped(vec![(3, [4096, 0, 0]), (2, [2048, 0, 0])]));
}

#[test]
fn position_sync_carries_the_path_in_full_precision() {
    let linear = EntityPositionSync { entity: VarInt(1), path: PositionPath::Linear([1.0, 2.0, 3.0]), y_rot: 90.0, x_rot: -10.0, on_ground: true };
    let doubles = cat(&[&1.0f64.to_be_bytes(), &2.0f64.to_be_bytes(), &3.0f64.to_be_bytes()]);
    assert_eq!(round_trip(linear), cat(&[&[1, 0], &doubles, &90.0f32.to_be_bytes(), &(-10.0f32).to_be_bytes(), &[1]]));
    let stepped = EntityPositionSync { entity: VarInt(1), path: PositionPath::Stepped(vec![([1.0, 2.0, 3.0], 5)]), y_rot: 0.0, x_rot: 0.0, on_ground: false };
    assert_eq!(round_trip(stepped)[..3], [1, 1, 1]);
}

#[test]
fn teleport_is_position_move_rotation_then_big_endian_relatives_then_ground() {
    let change = PositionMoveRotation { x: 1.0, y: 2.0, z: 3.0, delta_x: 0.0, delta_y: 0.5, delta_z: 0.0, y_rot: 90.0, x_rot: 0.0 };
    let bytes = round_trip(TeleportEntity { entity: VarInt(2), change, relatives: 0x101, on_ground: true });
    assert_eq!(bytes.len(), 1 + 6 * 8 + 2 * 4 + 4 + 1);
    assert_eq!(bytes[bytes.len() - 5..], [0, 0, 1, 1, 1]);
}

#[test]
fn motion_and_head_rotation() {
    assert_eq!(round_trip(SetEntityMotion { entity: VarInt(3), movement: LpVec3([0.0; 3]) }), [3, 0]);
    assert_eq!(round_trip(RotateHead { entity: VarInt(3), y_head_rot: 64 }), [3, 64]);
}

#[test]
fn status_packets() {
    assert_eq!(round_trip(EntityEvent { entity: 258, event: 9 }), [0, 0, 1, 2, 9]);
    assert_eq!(round_trip(Animate { entity: VarInt(1), action: 1 }), [1, 1]);
    assert_eq!(round_trip(HurtAnimation { entity: VarInt(1), yaw: 1.0 }), cat(&[&[1], &1.0f32.to_be_bytes()]));
    assert_eq!(round_trip(TakeItemEntity { item: VarInt(5), collector: VarInt(6), amount: VarInt(2) }), [5, 6, 2]);
}

#[test]
fn damage_event_shifts_optional_entity_ids_by_one() {
    let packet = DamageEvent {
        entity: VarInt(4),
        damage_type: VarInt(9),
        cause: OptEntityId(Some(7)),
        direct: OptEntityId(None),
        source_position: Optional(Some(Vec3 { x: 1.0, y: 2.0, z: 3.0 })),
    };
    let bytes = round_trip(packet);
    assert_eq!(bytes[..5], [4, 9, 8, 0, 1]);
    assert_eq!(bytes.len(), 5 + 24);
    assert_eq!(round_trip(DamageEvent::default()), [0, 0, 0, 0, 0]);
}

#[test]
fn equipment_marks_every_slot_but_the_last_with_bit_7() {
    let packet = SetEquipment {
        entity: VarInt(1),
        slots: vec![(EquipmentSlot::MainHand, Some(ItemStack::new(3, 2))), (EquipmentSlot::Head, None), (EquipmentSlot::Saddle, None)],
    };
    assert_eq!(round_trip(packet), [1, 0x80, 2, 3, 0, 0, 0x85, 0, 7, 0]);
    assert!(SetEquipment::read(&mut &[1, 0x88, 0][..]).is_err(), "slot 8 does not exist");
}

#[test]
fn mob_effects_and_attributes() {
    let effect = UpdateMobEffect { entity: VarInt(1), effect: VarInt(2), amplifier: VarInt(3), duration: VarInt(200), flags: EFFECT_VISIBLE | EFFECT_SHOW_ICON };
    assert_eq!(round_trip(effect), [1, 2, 3, 0xC8, 0x01, 6]);
    assert_eq!(round_trip(RemoveMobEffect { entity: VarInt(1), effect: VarInt(2) }), [1, 2]);
    let attributes = UpdateAttributes {
        entity: VarInt(1),
        attributes: vec![AttributeSnapshot {
            attribute: VarInt(4),
            base: 0.5,
            modifiers: vec![AttributeModifier { id: "a:b".into(), amount: 2.0, operation: AttributeModifierOperation::AddMultipliedBase }],
        }],
    };
    let bytes = round_trip(attributes);
    assert_eq!(bytes[..4], [1, 1, 4, 0x3F]);
    assert_eq!(bytes[bytes.len() - 1], 1, "add_multiplied_base");
    let mut too_many = vec![1];
    crate::write_varint(129, &mut too_many);
    assert!(UpdateAttributes::read(&mut &too_many[..]).is_err());
}

fn full_entry() -> PlayerInfoEntry {
    PlayerInfoEntry {
        uuid: 0x0123_4567_89AB_CDEF_0011_2233_4455_6677,
        name: "Bob".into(),
        game_mode: GameMode::Creative,
        listed: true,
        latency: 150,
        list_order: -2,
        show_hat: true,
        ..PlayerInfoEntry::default()
    }
}

/// The bridge's `hand_frames` test reads the same eight codecs in the same order.
#[test]
fn player_info_writes_every_action_in_declaration_order() {
    let all = ADD_PLAYER | INITIALIZE_CHAT | UPDATE_GAME_MODE | UPDATE_LISTED | UPDATE_LATENCY | UPDATE_DISPLAY_NAME | UPDATE_LIST_ORDER | UPDATE_HAT;
    let bytes = round_trip(PlayerInfoUpdate { actions: all, entries: vec![full_entry()] });
    let uuid = 0x0123_4567_89AB_CDEF_0011_2233_4455_6677u128.to_be_bytes();
    let tail = [3, b'B', b'o', b'b', 0, 0, 1, 1, 0x96, 0x01, 0, 0xFE, 0xFF, 0xFF, 0xFF, 0x0F, 1];
    assert_eq!(bytes, cat(&[&[0xFF, 1], &uuid, &tail]));
}

#[test]
fn player_info_leaves_unselected_fields_off_the_wire() {
    let entry = PlayerInfoEntry { uuid: 5, listed: true, latency: 7, ..PlayerInfoEntry::default() };
    let bytes = round_trip(PlayerInfoUpdate { actions: UPDATE_LISTED | UPDATE_LATENCY, entries: vec![entry] });
    assert_eq!(bytes.len(), 2 + 16 + 1 + 1);
    assert_eq!(bytes[0], 0b11000);
}

#[test]
fn player_info_carries_skins_chat_sessions_and_display_names() {
    let entry = PlayerInfoEntry {
        uuid: 1,
        name: "Al".into(),
        properties: vec![ProfileProperty { name: "textures".into(), value: "v".into(), signature: Some("s".into()) }],
        chat_session: Optional(Some(ChatSession { session_id: 2, expires_at_millis: 3, public_key: vec![9; 4], key_signature: vec![8; 2] })),
        display_name: Optional(Some(Component::text("Al"))),
        ..PlayerInfoEntry::default()
    };
    round_trip(PlayerInfoUpdate { actions: ADD_PLAYER | INITIALIZE_CHAT | UPDATE_DISPLAY_NAME, entries: vec![entry] });
    assert_eq!(round_trip(PlayerInfoRemove { uuids: vec![1, 2] }).len(), 1 + 32);
}

#[test]
fn serverbound_interactions() {
    let interact = Interact { entity: VarInt(300), hand: Hand::Off, location: LpVec3([0.0; 3]), using_secondary_action: true };
    assert_eq!(round_trip(interact), [0xAC, 0x02, 1, 0, 1]);
    assert_eq!(round_trip(Attack { entity: VarInt(5) }), [5]);
    assert_eq!(round_trip(Punch {}), Vec::<u8>::new());
    assert_eq!(round_trip(UseItem { hand: Hand::Main, sequence: VarInt(4), y_rot: 0.0, x_rot: 0.0 }), [0, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(round_trip(SetCarriedItem { slot: 8 }), [0, 8]);
    assert_eq!(round_trip(PickItemFromEntity { entity: VarInt(2), include_data: true }), [2, 1]);
    assert_eq!(round_trip(TeleportToEntity { uuid: 1 }).len(), 16);
}

#[test]
fn packet_ids_exist_in_the_play_tables() {
    use crate::ids::play::{clientbound as c, serverbound as s};
    assert_eq!(
        [c::ADD_ENTITY, c::REMOVE_ENTITIES, c::SET_ENTITY_DATA, c::MOVE_ENTITY_POS, c::MOVE_ENTITY_POS_ROT, c::MOVE_ENTITY_ROT, c::ENTITY_POSITION_SYNC],
        [1, 78, 101, 54, 55, 57, 35]
    );
    assert_eq!([c::TELEPORT_ENTITY, c::SET_ENTITY_MOTION, c::ROTATE_HEAD, c::ENTITY_EVENT, c::ANIMATE, c::SET_EQUIPMENT], [128, 103, 85, 34, 2, 104]);
    assert_eq!([s::INTERACT, s::ATTACK, s::PUNCH, s::USE_ITEM, s::SET_CARRIED_ITEM], [26, 1, 46, 67, 54]);
}
