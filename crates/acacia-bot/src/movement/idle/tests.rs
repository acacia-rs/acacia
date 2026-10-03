use super::*;
use acacia_client::proto::packets::PlayerAuthInputPlayMode;
use acacia_client::proto::types::{InputData, Vec3f};

fn player(y: f32) -> PlayerState {
    PlayerState { position: Vec3f { x: 1.0, y, z: 2.0 }, alive: true, ..PlayerState::default() }
}

#[test]
fn stands_still_like_vanilla_and_acknowledges_server_moves() {
    let mut idle = Idle::default();
    let first = idle.tick(&player(64.0), false).unwrap();
    assert_eq!(first.tick, FIRST_TICK);
    assert_eq!(first.play_mode, PlayerAuthInputPlayMode::Normal);
    assert_eq!(first.position.y, 64.0 + super::super::EYE_HEIGHT);
    assert_eq!((first.delta.x, first.delta.y, first.delta.z), (0.0, -0.0784, 0.0));
    // Vanilla's standing-still flags, exactly (a bitset on the wire, so order is irrelevant).
    assert_eq!(first.input_data.len(), 2);
    assert!([InputData::VerticalCollision, InputData::BlockBreakingDelayEnabled].iter().all(|f| first.input_data.contains(f)));

    let moved = idle.tick(&player(70.0), true).unwrap();
    assert_eq!(moved.play_mode, PlayerAuthInputPlayMode::Screen);
    assert!(moved.input_data.contains(&InputData::HandledTeleport));
    assert!(!idle.tick(&player(70.0), true).unwrap().input_data.contains(&InputData::HandledTeleport));

    let in_place = PlayerState { teleports: 1, ..player(70.0) };
    assert!(idle.tick(&in_place, true).unwrap().input_data.contains(&InputData::HandledTeleport), "teleport to the same spot");
    assert!(!idle.tick(&in_place, true).unwrap().input_data.contains(&InputData::HandledTeleport));

    let dead = PlayerState { alive: false, ..player(70.0) };
    assert!(idle.tick(&dead, true).is_none());
    assert_eq!(idle.tick(&in_place, true).unwrap().tick, FIRST_TICK + 6, "tick keeps counting while dead");
}

#[test]
fn looks_where_told_until_the_server_moves_it() {
    let mut idle = Idle::default();
    let turned_to = |teleports| PlayerState { yaw: 30.0, pitch: 10.0, teleports, ..player(64.0) };
    let p = turned_to(0);
    idle.tick(&p, true);
    let eye = [1.0, 64.0 + super::super::EYE_HEIGHT, 2.0];
    idle.look_at(eye, [eye[0] + 5.0, eye[1] - 5.0, eye[2]]);
    let (yaw, pitch) = idle.facing(&p);
    assert!((yaw + 90.0).abs() < 1e-3 && (pitch - 45.0).abs() < 1e-3, "+x and down: {yaw} {pitch}");
    let turned = idle.tick(&p, true).unwrap();
    assert_eq!((turned.yaw, turned.pitch, turned.head_yaw), (yaw, pitch, yaw));
    assert_eq!(idle.tick_facing(&p, 5.0, 6.0, true).unwrap().yaw, 5.0, "an explicit rotation wins");

    let teleported = idle.tick(&turned_to(1), true).unwrap();
    assert_eq!((teleported.yaw, teleported.pitch), (30.0, 10.0), "the server's rotation after a move");
}

#[test]
fn stepping_off_a_vehicle_is_not_a_server_move() {
    let mut idle = Idle::default();
    idle.tick(&player(64.0), true);
    idle.stepped_to([1.0, 65.0, 2.0]);
    assert!(!idle.tick(&player(65.0), true).unwrap().input_data.contains(&InputData::HandledTeleport));
}
