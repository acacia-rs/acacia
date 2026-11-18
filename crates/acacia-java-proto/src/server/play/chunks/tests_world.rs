use super::effects::*;
use super::particles::*;
use super::world::*;
use super::world_border::*;
use crate::actions::BlockPos;
use crate::registry::PARTICLE_TYPE;
use crate::server::round_trip;
use crate::types::{GameMode, GlobalPos, OptionalGameMode, Vec3};
use crate::{Error, VarInt, Wire};

fn spawn() -> SpawnInfo {
    SpawnInfo {
        dimension_type: VarInt(0),
        dimension: "minecraft:overworld".into(),
        seed: 0x0102_0304_0506_0708,
        game_type: GameMode::Creative,
        previous_game_type: OptionalGameMode(Some(GameMode::Survival)),
        is_debug: false,
        is_flat: true,
        last_death: Some(GlobalPos { dimension: "minecraft:the_nether".into(), pos: BlockPos([1, 2, 3]) }),
        portal_cooldown: VarInt(5),
        sea_level: VarInt(63),
    }
}

fn string(out: &mut Vec<u8>, s: &str) {
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
}

#[test]
fn login_is_the_vanilla_layout_with_a_var_int_previous_game_mode() {
    let mut info = spawn();
    info.last_death = None;
    info.previous_game_type = OptionalGameMode(None);
    let packet = Login {
        player_id: 7,
        hardcore: false,
        levels: vec!["minecraft:overworld".into()],
        max_players: VarInt(20),
        chunk_radius: VarInt(10),
        simulation_distance: VarInt(8),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        spawn: info,
        online_mode: false,
        enforces_secure_chat: true,
    };
    let mut want = vec![0, 0, 0, 7, 0, 1];
    string(&mut want, "minecraft:overworld");
    want.extend([20, 10, 8, 0, 1, 0]);
    want.extend([0]);
    string(&mut want, "minecraft:overworld");
    want.extend(0x0102_0304_0506_0708i64.to_be_bytes());
    // game type 1; previous none = 0; debug 0, flat 1; no death location; cooldown 5; sea level 63; online 0, secure 1
    want.extend([1, 0, 0, 1, 0, 5, 63, 0, 1]);
    assert_eq!(round_trip(packet), want);
}

#[test]
fn previous_game_mode_is_id_plus_one() {
    for (previous, byte) in [(None, 0), (Some(GameMode::Survival), 1), (Some(GameMode::Spectator), 4)] {
        let mut info = spawn();
        info.previous_game_type = OptionalGameMode(previous);
        let bytes = round_trip(info);
        // type 0, "minecraft:overworld", seed, game type 1, then the previous mode.
        assert_eq!(bytes[1 + 1 + 19 + 8 + 1], byte);
    }
}

#[test]
fn respawn_and_spawn_info_round_trip() {
    let bytes = round_trip(Respawn { spawn: spawn(), data_to_keep: KEEP_ATTRIBUTES | KEEP_ENTITY_DATA });
    assert_eq!(*bytes.last().unwrap(), 3);
}

#[test]
fn game_event_and_time() {
    assert_eq!(round_trip(GameEvent { event: game_event::LEVEL_CHUNKS_LOAD_START, param: 0.0 }), [13, 0, 0, 0, 0]);
    assert_eq!(round_trip(GameEvent { event: game_event::CHANGE_GAME_MODE, param: 1.0 })[1..], 1f32.to_be_bytes());
    let bytes = round_trip(SetTime {
        game_time: 6000,
        clocks: vec![ClockUpdate { clock: VarInt(0), state: ClockState { total_ticks: super::VarLong(300), partial_tick: 0.5, rate: 1.0 } }],
    });
    // long game time, map size 1, clock id 0, VarLong 300 (0xAC 0x02), float, float.
    assert_eq!(bytes[8..], [1, 0, 0xAC, 0x02, 0x3F, 0, 0, 0, 0x3F, 0x80, 0, 0]);
}

#[test]
fn default_spawn_and_player_position() {
    let bytes = round_trip(SetDefaultSpawnPosition { pos: GlobalPos { dimension: "minecraft:overworld".into(), pos: BlockPos([0, 64, 0]) }, yaw: 90.0, pitch: 0.0 });
    assert_eq!(bytes.len(), 1 + 19 + 8 + 4 + 4);
    let change = PositionMoveRotation { x: 1.0, y: 2.0, z: 3.0, y_rot: 90.0, x_rot: -10.0, ..Default::default() };
    let bytes = round_trip(PlayerPosition { id: VarInt(9), change, relatives: relative::X | relative::ROTATE_DELTA });
    assert_eq!((bytes[0], bytes.len()), (9, 1 + 6 * 8 + 8 + 4));
    assert_eq!(bytes[bytes.len() - 4..], [0, 0, 1, 1], "a big-endian int, bit = Relative ordinal");
}

#[test]
fn border_packets() {
    let bytes = round_trip(InitializeBorder {
        center_x: 0.0,
        center_z: 0.0,
        size: 60.0,
        lerp_target: 60.0,
        lerp_time: super::VarLong(0),
        absolute_max_size: VarInt(29_999_984),
        warning_blocks: VarInt(5),
        warning_time: VarInt(15),
    });
    assert_eq!(bytes.len(), 4 * 8 + 1 + 4 + 1 + 1);
    round_trip(SetBorderCenter { x: 1.5, z: -2.5 });
    round_trip(SetBorderLerpSize { old_size: 10.0, new_size: 20.0, lerp_time: super::VarLong(5000) });
    round_trip(SetBorderSize { size: 7.0 });
    assert_eq!(round_trip(SetBorderWarningDelay { seconds: VarInt(15) }), [15]);
    assert_eq!(round_trip(SetBorderWarningDistance { blocks: VarInt(5) }), [5]);
}

#[test]
fn level_event_is_int_pos_int_bool() {
    let bytes = round_trip(LevelEvent { event: 2001, pos: BlockPos([1, 2, 3]), data: 5, global: true });
    assert_eq!((bytes[..4].to_vec(), bytes.len(), bytes[bytes.len() - 1]), (2001i32.to_be_bytes().to_vec(), 17, 1));
}

#[test]
fn sound_holders_and_stop_sound() {
    let sound = Sound { sound: SoundRef::Id(5), source: SoundSource::Ui, x: sound_coord(1.5), y: sound_coord(-2.0), z: 0, volume: 1.0, pitch: 0.5, seed: -1 };
    let bytes = round_trip(sound);
    assert_eq!(bytes[..2], [6, 10], "registry id + 1, then the source ordinal (Ui is 10)");
    assert_eq!(bytes[2..6], 12i32.to_be_bytes());
    let inline = round_trip(SoundEntity { sound: SoundRef::Inline { name: "x:y".into(), fixed_range: Some(16.0) }, source: SoundSource::Players, entity: VarInt(300), volume: 1.0, pitch: 1.0, seed: 3 });
    assert_eq!(inline[..8], [0, 3, b'x', b':', b'y', 1, 0x41, 0x80], "0, identifier, optional float");
    round_trip(SoundEntity { sound: SoundRef::Inline { name: "x:y".into(), fixed_range: None }, ..SoundEntity::default() });
    assert_eq!(round_trip(StopSound::default()), [0]);
    assert_eq!(round_trip(StopSound { source: Some(SoundSource::Music), name: None }), [1, 1]);
    assert_eq!(round_trip(StopSound { source: None, name: Some("a".into()) }), [2, 1, b'a']);
    assert_eq!(round_trip(StopSound { source: Some(SoundSource::Master), name: Some("a".into()) }), [3, 0, 1, b'a']);
}

fn level_particles(particle: Particle) -> LevelParticles {
    LevelParticles {
        particle,
        always_show: true,
        x: 1.0,
        y: 2.0,
        z: 3.0,
        x_dist: 0.5,
        y_dist: 0.25,
        z_dist: 0.125,
        x_max_speed: 0.05,
        y_max_speed: 0.05,
        z_max_speed: 0.05,
        count: VarInt(300),
        ..Default::default()
    }
}

/// The tail after the particle: two flags, three doubles, six floats, a count of 300 and randomization 0.
const TAIL: usize = 2 + 24 + 24 + 2 + 1;

#[test]
fn level_particles_is_the_vanilla_layout() {
    let heart = Particle::named("heart", Options::None).unwrap();
    let id = PARTICLE_TYPE.id("minecraft:heart").unwrap() as u8;
    let bytes = round_trip(level_particles(heart));
    assert_eq!(bytes[..3], [id, 0, 1], "particle first, then override_limiter and always_show");
    assert_eq!(bytes.len(), 1 + TAIL);
    assert_eq!(bytes[bytes.len() - 3..], [0xAC, 0x02, 0], "count, then randomization");
    assert_eq!(bytes[1 + 2 + 24 + 12..1 + 2 + 24 + 24], 0.05f32.to_be_bytes().repeat(3));
}

fn options_of(particle: Particle) -> Vec<u8> {
    let bytes = round_trip(level_particles(particle));
    let kind_len = if bytes[0] & 0x80 != 0 { 2 } else { 1 };
    bytes[kind_len..bytes.len() - TAIL].to_vec()
}

#[test]
fn options_use_vanilla_layouts() {
    let named = |name, options| Particle::named(name, options).unwrap();
    assert_eq!(options_of(named("dragon_breath", Options::Power(1.0))), 1.0f32.to_be_bytes());
    assert_eq!(options_of(named("effect", Options::Spell { argb: -1, power: 1.0 })), [&[0xFF; 4][..], &1.0f32.to_be_bytes()].concat());
    assert_eq!(options_of(named("instant_effect", Options::Spell { argb: -1, power: 1.0 })).len(), 8);
    assert_eq!(options_of(named("flash", Options::Color(-1))), [0xFF; 4]);
    assert_eq!(options_of(named("tinted_leaves", Options::Color(0x48B518))), [0, 0x48, 0xB5, 0x18]);
    assert_eq!(options_of(named("entity_effect", Options::Color(-16777216))), [0xFF, 0, 0, 0]);
    assert_eq!(options_of(named("dust_pillar", Options::BlockState(1))), [1]);
    assert_eq!(options_of(named("block_crumble", Options::BlockState(300))), [0xAC, 0x02]);
    assert_eq!(options_of(named("dust", Options::Dust { rgb: 0xFF0000, scale: 1.0 })), [&[0, 0xFF, 0, 0][..], &1.0f32.to_be_bytes()].concat());
    assert_eq!(options_of(named("dust_color_transition", Options::DustTransition { from: 1, to: 2, scale: 1.0 })).len(), 12);
    let trail = named("trail", Options::Trail { target: Vec3 { x: 1.0, y: 2.0, z: 3.0 }, rgb: 0xFFFFFF, duration: 20 });
    assert_eq!(options_of(trail).len(), 24 + 4 + 1);
    assert_eq!(options_of(named("shriek", Options::Shriek(5))), [5]);
    assert_eq!(options_of(named("sculk_charge", Options::SculkCharge(0.5))), 0.5f32.to_be_bytes());
    assert_eq!(options_of(named("geyser", Options::Geyser(3))), 3i32.to_be_bytes());
    assert_eq!(options_of(named("geyser_poof", Options::GeyserBase { int: 3, float: 1.0 })).len(), 8);
    assert_eq!(options_of(named("heart", Options::None)), []);
}

#[test]
fn item_particle_writes_item_before_count() {
    let item = ItemTemplate { item: VarInt(300), count: VarInt(3), ..Default::default() };
    // Item id, count, then an empty patch (two zero counts); a stack has the count first.
    assert_eq!(options_of(Particle::named("item", Options::Item(item)).unwrap()), [0xAC, 0x02, 3, 0, 0]);
}

#[test]
fn vibration_sources() {
    let block = Options::Vibration { source: PositionSource::Block(BlockPos([1, 2, 3])), arrival_ticks: 40 };
    assert_eq!(options_of(Particle::named("vibration", block).unwrap()).len(), 1 + 8 + 1);
    let entity = Options::Vibration { source: PositionSource::Entity { id: 9, y_offset: 1.5 }, arrival_ticks: 40 };
    assert_eq!(options_of(Particle::named("vibration", entity).unwrap()), [1, 9, 0x3F, 0xC0, 0, 0, 40]);
}

#[test]
fn options_must_fit_the_particle_type() {
    assert_eq!(Particle::named("heart", Options::Power(1.0)), Err(Error::Invalid("particle options")));
    assert_eq!(Particle::named("dust", Options::None), Err(Error::Invalid("particle options")));
    assert_eq!(Particle::named("no_such_particle", Options::None), Err(Error::Invalid("particle type")));
    assert!(Particle::read(&mut &[0xC8, 0x01][..]).is_err(), "id 200 is past the registry");
}

#[test]
fn every_particle_type_round_trips() {
    for id in 0..PARTICLE_TYPE.len() as i32 {
        let mut bytes = Vec::new();
        VarInt(id).write(&mut bytes);
        // Options are at most a few dozen bytes; zero-filled they decode for every shape that has no registry lookup.
        bytes.extend([0u8; 64]);
        let particle = Particle::read(&mut &bytes[..]).unwrap();
        let mut again = Vec::new();
        particle.write(&mut again);
        let mut input = &again[..];
        assert_eq!(Particle::read(&mut input).unwrap(), particle, "{}", PARTICLE_TYPE.name(id).unwrap());
    }
}
