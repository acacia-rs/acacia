use acacia_client::proto::packets::{AddEntity, SetEntityData, SetEntityLink};
use acacia_client::proto::types::{
    EntityProperties, Link, MetadataDictionaryItem, MetadataDictionaryItemKey, MetadataDictionaryItemType,
    MetadataDictionaryItemValue, MetadataDictionaryItemValueDefault,
};
use acacia_client::proto::RawPacket;

use super::*;
use crate::movement::Idle;
use crate::state::queries::test_support::{fixtures, raw};
use crate::state::PlayerState;

const ME_RUNTIME: u64 = 1;
const ME_UNIQUE: i64 = -1;

fn v(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f { x, y, z }
}

fn state() -> GameState {
    let mut s = GameState::default();
    (s.player.runtime_entity_id, s.player.unique_entity_id) = (ME_RUNTIME, ME_UNIQUE);
    s.player.position = v(0.0, 70.0, 0.0);
    s
}

fn spawn(state: &mut GameState, runtime_id: u64, unique_id: i64, kind: &str, pos: Vec3f) {
    let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    (p.runtime_id, p.unique_id, p.entity_type, p.position, p.links) = (runtime_id, unique_id, kind.into(), pos, vec![]);
    (p.pitch, p.yaw) = (0.0, 135.0);
    state.apply(&raw(&p)).unwrap();
}

fn link(state: &mut GameState, vehicle: i64, r#type: u8) {
    let link = Link { ridden_entity_id: vehicle, rider_entity_id: ME_UNIQUE, r#type, immediate: false, rider_initiated: true, angular_velocity: 0.0 };
    state.apply(&raw(&SetEntityLink { link })).unwrap();
}

fn seat_offset(offset: Vec3f) -> RawPacket {
    let item = MetadataDictionaryItem {
        key: MetadataDictionaryItemKey::RiderSeatPosition,
        r#type: MetadataDictionaryItemType::Vec3f,
        legacy_type: 0,
        value: MetadataDictionaryItemValue::Default(MetadataDictionaryItemValueDefault::Vec3f(offset)),
    };
    raw(&SetEntityData { runtime_entity_id: ME_RUNTIME, metadata: vec![item], properties: EntityProperties { ints: vec![], floats: vec![] }, tick: 0 })
}

fn standing(player: &PlayerState) -> PlayerAuthInput {
    let mut idle = Idle::default();
    idle.tick(player, true).unwrap()
}

fn round_trip(input: &PlayerAuthInput) -> PlayerAuthInput {
    let back: PlayerAuthInput = raw(input).decode().unwrap();
    assert_eq!(&back, input);
    back
}

#[test]
fn passenger_of_unpredicted_vehicle_reports_its_seat() {
    let mut s = state();
    spawn(&mut s, 9, 90, "minecraft:minecart", v(10.0, 64.0, 10.0));
    s.apply(&seat_offset(v(0.0, 1.5, 0.0))).unwrap();
    link(&mut s, 90, 1);
    let seat = Seat::of(&s).unwrap();
    assert_eq!(seat, Seat { position: v(10.0, 65.5, 10.0), predicted: None });

    // Vanilla (pig): the first seated tick keeps the standing delta, later ones report none.
    let mut first = standing(&s.player);
    apply_seat(&mut first, &seat, true);
    assert_eq!(first.delta.y, standing(&s.player).delta.y);

    let mut input = standing(&s.player);
    apply_seat(&mut input, &seat, false);
    let input = round_trip(&input);
    assert_eq!(input.input_data.len(), 2);
    assert!(input.input_data.contains(&F::BlockBreakingDelayEnabled) && input.input_data.contains(&F::VerticalCollision));
    assert_eq!((input.position.clone(), input.delta.y), (v(10.0, 65.5, 10.0), 0.0));
    assert!(input.vehicle_rotation.is_none() && input.predicted_vehicle.is_none());
}

#[test]
fn driver_of_predicted_vehicle_reports_the_vehicle_from_the_first_seated_tick() {
    let mut s = state();
    spawn(&mut s, 7, 70, "minecraft:boat", v(5.0, 62.5, 5.0));
    link(&mut s, 70, 1);
    let seat = Seat::of(&s).unwrap();
    assert_eq!(seat, Seat { position: v(5.0, 62.5, 5.0), predicted: Some((70, Vec2f { x: 0.0, z: 135.0 })) });

    let mut first = standing(&s.player);
    apply_seat(&mut first, &seat, true);
    let first = round_trip(&first);
    assert!(first.input_data.contains(&F::ClientPredictedVehicle) && first.input_data.contains(&F::VerticalCollision));
    assert_eq!((first.predicted_vehicle, first.vehicle_rotation), (Some(70), Some(Vec2f { x: 0.0, z: 135.0 })));
    assert_eq!(first.delta, v(0.0, 0.0, 0.0));

    link(&mut s, 70, 2);
    assert_eq!(Seat::of(&s).unwrap().predicted, None, "a passenger seat predicts nothing");
}

#[test]
fn dismount_tick_stands_at_the_exit_without_sneak_or_vehicle_fields() {
    let mut s = state();
    spawn(&mut s, 7, 70, "minecraft:boat", v(5.0, 64.0, 5.0));
    link(&mut s, 70, 1);
    let seat = Seat::of(&s).unwrap();
    let mut input = standing(&s.player);
    apply_seat(&mut input, &seat, false);
    apply_dismount(&mut input, v(5.0, 64.0 + EYE_HEIGHT, 4.0));
    let input = round_trip(&input);
    assert_eq!(input.position, v(5.0, 64.0 + EYE_HEIGHT, 4.0));
    assert_eq!(input.input_data, vec![F::BlockBreakingDelayEnabled]);
    assert!(input.predicted_vehicle.is_none() && input.vehicle_rotation.is_none());
}

#[test]
fn unknown_vehicle_falls_back_to_the_server_position() {
    let mut s = state();
    link(&mut s, 123, 1);
    let seat = Seat::of(&s).unwrap();
    assert_eq!(seat, Seat { position: s.player.eye_position(), predicted: None });
}

#[test]
fn leaving_a_seat_puts_the_feet_below_the_eye_but_a_predicted_vehicle_s_position_is_the_feet() {
    // Live on BDS: subtracting the eye height from a boat's position left the bot a block underground.
    let boat = Seat { position: v(1.5, -59.4, -0.5), predicted: Some((70, Vec2f { x: 0.0, z: 0.0 })) };
    assert_eq!(boat.leaving_feet(), [1.5, -59.4, -0.5]);
    let minecart = Seat { position: v(10.0, 65.62, 10.0), predicted: None };
    assert_eq!(minecart.leaving_feet()[1], 65.62 - EYE_HEIGHT);
}
