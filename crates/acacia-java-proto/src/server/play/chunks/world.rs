//! Level and dimension packets: `Login`, `Respawn`, game events, time and the default spawn.
//! azalea writes the previous game mode as an i8 here; vanilla's `GameType.OPTIONAL_STREAM_CODEC` is a VarInt.

#![allow(unused_doc_comments)] // docs on `packet!` fields' structs

use super::{VarLong, read_option, write_option};
use crate::packets::packet;
use crate::types::{GameMode, GlobalPos, OptionalGameMode};
use crate::{Error, VarInt, Wire};

pub use crate::packets::{PlayerPosition, PositionMoveRotation};

/// `Relative` constants as bits of `PlayerPosition::relatives` (a big-endian int, bit = ordinal).
pub mod relative {
    pub const X: i32 = 1;
    pub const Y: i32 = 1 << 1;
    pub const Z: i32 = 1 << 2;
    pub const Y_ROT: i32 = 1 << 3;
    pub const X_ROT: i32 = 1 << 4;
    pub const DELTA_X: i32 = 1 << 5;
    pub const DELTA_Y: i32 = 1 << 6;
    pub const DELTA_Z: i32 = 1 << 7;
    pub const ROTATE_DELTA: i32 = 1 << 8;
}

/// `CommonPlayerSpawnInfo`. `dimension_type` is the dimension type's registry id from the configuration sync;
/// `dimension` the level's identifier.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpawnInfo {
    pub dimension_type: VarInt,
    pub dimension: String,
    pub seed: i64,
    pub game_type: GameMode,
    pub previous_game_type: OptionalGameMode,
    pub is_debug: bool,
    pub is_flat: bool,
    pub last_death: Option<GlobalPos>,
    pub portal_cooldown: VarInt,
    pub sea_level: VarInt,
}

impl Wire for SpawnInfo {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            dimension_type: VarInt::read(input)?,
            dimension: String::read(input)?,
            seed: i64::read(input)?,
            game_type: GameMode::read(input)?,
            previous_game_type: OptionalGameMode::read(input)?,
            is_debug: bool::read(input)?,
            is_flat: bool::read(input)?,
            last_death: read_option(input)?,
            portal_cooldown: VarInt::read(input)?,
            sea_level: VarInt::read(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.dimension_type.write(out);
        self.dimension.write(out);
        self.seed.write(out);
        self.game_type.write(out);
        self.previous_game_type.write(out);
        self.is_debug.write(out);
        self.is_flat.write(out);
        write_option(&self.last_death, out);
        self.portal_cooldown.write(out);
        self.sea_level.write(out);
    }
}

packet!(Login {
    player_id: i32,
    hardcore: bool,
    levels: Vec<String>,
    max_players: VarInt,
    chunk_radius: VarInt,
    simulation_distance: VarInt,
    reduced_debug_info: bool,
    show_death_screen: bool,
    do_limited_crafting: bool,
    spawn: SpawnInfo,
    online_mode: bool,
    enforces_secure_chat: bool,
});

/// `data_to_keep` bits: [`KEEP_ATTRIBUTES`], [`KEEP_ENTITY_DATA`]. The client starts a new, empty level.
packet!(Respawn { spawn: SpawnInfo, data_to_keep: u8 });
pub const KEEP_ATTRIBUTES: u8 = 1;
pub const KEEP_ENTITY_DATA: u8 = 2;

/// `ClientboundGameEventPacket`: a type byte and a float parameter.
packet!(GameEvent { event: u8, param: f32 });

/// `ClientboundGameEventPacket.Type` ids.
pub mod game_event {
    pub const NO_RESPAWN_BLOCK_AVAILABLE: u8 = 0;
    pub const START_RAINING: u8 = 1;
    pub const STOP_RAINING: u8 = 2;
    pub const CHANGE_GAME_MODE: u8 = 3;
    pub const WIN_GAME: u8 = 4;
    pub const DEMO_EVENT: u8 = 5;
    pub const PLAY_ARROW_HIT_SOUND: u8 = 6;
    pub const RAIN_LEVEL_CHANGE: u8 = 7;
    pub const THUNDER_LEVEL_CHANGE: u8 = 8;
    pub const PUFFER_FISH_STING: u8 = 9;
    pub const GUARDIAN_ELDER_EFFECT: u8 = 10;
    pub const IMMEDIATE_RESPAWN: u8 = 11;
    pub const LIMITED_CRAFTING: u8 = 12;
    pub const LEVEL_CHUNKS_LOAD_START: u8 = 13;
}

packet!(ClockState { total_ticks: VarLong, partial_tick: f32, rate: f32 });
packet!(ClockUpdate { clock: VarInt, state: ClockState });

/// `ClientboundSetTimePacket`: the game time, then a map from world clock registry id to its state (a VarInt
/// size, then pairs).
packet!(SetTime { game_time: i64, clocks: Vec<ClockUpdate> });

/// `LevelData.RespawnData.STREAM_CODEC`: the spawn's dimension and block, then yaw and pitch.
packet!(SetDefaultSpawnPosition { pos: GlobalPos, yaw: f32, pitch: f32 });
