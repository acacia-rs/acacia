use acacia_client::proto::packets::{MovePlayerMode, StartGameDimension};
use acacia_client::proto::types::{PlayerAttributes, Vec2f};

use super::*;
use crate::state::queries::test_support::{fixtures, raw};

fn v(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f { x, y, z }
}

fn approx(a: &Vec3f, b: &Vec3f) -> bool {
    (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4 && (a.z - b.z).abs() < 1e-4
}

fn started() -> PlayerState {
    let mut sg: StartGame = fixtures::<StartGame>()[0].decode().unwrap();
    sg.entity_id = -42;
    sg.runtime_entity_id = 7;
    sg.player_position = v(100.5, 65.62, -20.5);
    sg.rotation = Vec2f { x: 10.0, z: 90.0 };
    sg.player_gamemode = GameMode::Fallback;
    sg.world_gamemode = GameMode::Adventure;
    sg.dimension = StartGameDimension::Nether;
    sg.spawn_position = BlockCoordinates { x: 1, y: 2, z: 3 };
    let mut player = PlayerState::default();
    player.apply(&raw(&sg)).unwrap();
    player
}

fn attr(name: &str, current: f32, max: f32) -> PlayerAttributesItem {
    PlayerAttributesItem { min: 0.0, max, current, default_min: 0.0, default_max: max, default: max, name: name.into(), modifiers: vec![] }
}

fn attributes(runtime_entity_id: u64, attributes: PlayerAttributes) -> UpdateAttributes {
    UpdateAttributes { runtime_entity_id, attributes, tick: 0 }
}

#[test]
fn start_game_sets_identity_and_feet_position() {
    let p = started();
    assert_eq!((p.runtime_entity_id, p.unique_entity_id), (7, -42));
    assert!(approx(&p.position, &v(100.5, 64.0, -20.5)));
    assert!(approx(&p.eye_position(), &v(100.5, 65.62, -20.5)));
    assert_eq!((p.pitch, p.yaw), (10.0, 90.0));
    assert_eq!(p.game_mode, GameMode::Adventure);
    assert_eq!(p.dimension, 1);
    assert_eq!(p.spawn_position, Some(BlockCoordinates { x: 1, y: 2, z: 3 }));
}

#[test]
fn start_game_fixtures_decode_through_apply() {
    for packet in fixtures::<StartGame>() {
        let expected: StartGame = packet.decode().unwrap();
        let mut p = PlayerState::default();
        p.apply(&packet).unwrap();
        assert_eq!(p.runtime_entity_id, expected.runtime_entity_id);
    }
}

#[test]
fn moves_for_own_runtime_id_only() {
    let mut p = started();
    let mv = |runtime_id, y| MovePlayer {
        runtime_id,
        position: v(1.0, y, 2.0),
        pitch: 5.0,
        yaw: -45.0,
        head_yaw: -45.0,
        mode: MovePlayerMode::Normal,
        on_ground: true,
        ridden_runtime_id: 0,
        teleport: None,
        tick: 0,
    };
    p.apply(&raw(&mv(8, 100.0))).unwrap();
    assert!(approx(&p.position, &v(100.5, 64.0, -20.5)));
    p.apply(&raw(&mv(7, 71.62))).unwrap();
    assert!(approx(&p.position, &v(1.0, 70.0, 2.0)));
    assert_eq!((p.pitch, p.yaw), (5.0, -45.0));

    let correction = CorrectPlayerMovePrediction {
        prediction_type: CorrectPlayerMovePredictionPredictionType::Player,
        position: v(3.0, 11.62, 4.0),
        delta: v(0.0, 0.0, 0.0),
        rotation: Vec2f { x: 0.0, z: 0.0 },
        angular_velocity: None,
        on_ground: true,
        tick: 9,
    };
    p.apply(&raw(&correction)).unwrap();
    assert!(approx(&p.position, &v(3.0, 10.0, 4.0)));
}

#[test]
fn attributes_health_and_death() {
    let mut p = started();
    let own = vec![
        attr("minecraft:health", 14.0, 24.0),
        attr("minecraft:player.hunger", 17.0, 20.0),
        attr("minecraft:player.saturation", 2.5, 20.0),
        attr("minecraft:player.level", 30.0, 24791.0),
        attr("minecraft:player.experience", 0.25, 1.0),
        attr("minecraft:movement", 0.1, 3.4e38),
    ];
    p.apply(&raw(&attributes(8, vec![attr("minecraft:health", 1.0, 20.0)]))).unwrap();
    assert_eq!(p.health, 20.0);
    p.apply(&raw(&attributes(7, own))).unwrap();
    assert_eq!((p.health, p.max_health, p.hunger, p.saturation), (14.0, 24.0, 17.0, 2.5));
    assert_eq!((p.xp_level, p.xp_progress), (30, 0.25));
    assert!(p.alive);

    p.apply(&raw(&SetHealth { health: 0 })).unwrap();
    assert!(!p.alive);

    let respawn = |state| Respawn { position: v(0.5, 71.62, 0.5), state, runtime_entity_id: 0 };
    p.apply(&raw(&respawn(0))).unwrap();
    assert!(!p.alive);
    p.apply(&raw(&respawn(1))).unwrap();
    assert!(p.alive);
    assert_eq!(p.health, 24.0);
    assert!(approx(&p.position, &v(0.5, 70.0, 0.5)));
}

#[test]
fn game_mode_dimension_and_spawn() {
    let mut p = started();
    p.apply(&raw(&SetPlayerGameType { gamemode: GameMode::Creative })).unwrap();
    assert_eq!(p.game_mode, GameMode::Creative);
    let update = |id| UpdatePlayerGameType { gamemode: GameMode::Spectator, player_unique_id: id, tick: 0 };
    p.apply(&raw(&update(-1))).unwrap();
    assert_eq!(p.game_mode, GameMode::Creative);
    p.apply(&raw(&update(-42))).unwrap();
    assert_eq!(p.game_mode, GameMode::Spectator);

    let change = ChangeDimension { dimension: 2, position: v(100.0, 50.0, 0.0), respawn: false, loading_screen_id: Some(3) };
    p.apply(&raw(&change)).unwrap();
    assert_eq!(p.dimension, 2);
    assert!(approx(&p.position, &v(100.0, 50.0, 0.0)));

    let spawn = |spawn_type, x| SetSpawnPosition {
        spawn_type,
        player_position: BlockCoordinates { x, y: 64, z: 0 },
        dimension: 0,
        world_position: BlockCoordinates { x: 0, y: 0, z: 0 },
    };
    p.apply(&raw(&spawn(SetSpawnPositionSpawnType::World, 9))).unwrap();
    assert_eq!(p.spawn_position, Some(BlockCoordinates { x: 1, y: 2, z: 3 }));
    p.apply(&raw(&spawn(SetSpawnPositionSpawnType::Player, 9))).unwrap();
    assert_eq!(p.spawn_position, Some(BlockCoordinates { x: 9, y: 64, z: 0 }));
}
