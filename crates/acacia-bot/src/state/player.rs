use super::{Abilities, Effects};
use acacia_client::proto::packets::{
    ChangeDimension, CorrectPlayerMovePrediction, CorrectPlayerMovePredictionPredictionType, MobEffect, MovePlayer, Respawn,
    SetEntityData, SetHealth, SetPlayerGameType, SetSpawnPosition, SetSpawnPositionSpawnType, StartGame, UpdateAbilities,
    UpdateAttributes, UpdatePlayerGameType,
};
use acacia_client::proto::types::{
    BlockCoordinates, GameMode, MetadataDictionaryItemKey, MetadataDictionaryItemValue as Meta,
    MetadataDictionaryItemValueDefault, MetadataFlags2, PlayerAttributesItem, Vec3f,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// Respawn state sent by the server once the player has been placed at its respawn point.
const RESPAWN_READY: u8 = 1;
/// `PlayerFlags` metadata bit for sleeping (gophertunnel `EntityDataPlayerFlagSleep`).
const PLAYER_FLAG_SLEEP: i8 = 1 << 1;

/// The local player.
///
/// `position` is the FEET position. Bedrock sends player positions at eye height (feet +
/// [`Self::EYE_HEIGHT`]) in StartGame, MovePlayer, Respawn, ChangeDimension and
/// CorrectPlayerMovePrediction; the offset is removed on the way in.
#[derive(Debug)]
pub struct PlayerState {
    pub runtime_entity_id: u64,
    pub unique_entity_id: i64,
    pub position: Vec3f,
    /// Degrees.
    pub yaw: f32,
    pub pitch: f32,
    pub game_mode: GameMode,
    /// 0 overworld, 1 nether, 2 end.
    pub dimension: i32,
    pub spawn_position: Option<BlockCoordinates>,
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub saturation: f32,
    pub xp_level: i32,
    /// Progress towards the next level, 0..1.
    pub xp_progress: f32,
    pub alive: bool,
    /// Deaths seen this session (a death and respawn can both land between two polls).
    pub deaths: u32,
    pub effects: Effects,
    pub abilities: Abilities,
    /// `minecraft:movement` attribute: base walking speed with modifiers (sprinting, slowness, ...).
    pub movement_speed: f32,
    /// The sleeping entity flag (Geyser) and player flag (BDS, PocketMine) from `SetEntityData`.
    pub(crate) sleep_flags: [bool; 2],
    /// Own `MovePlayer`s and ready `Respawn`s seen: each needs `HandledTeleport` (movement::Idle).
    pub(crate) teleports: u32,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            runtime_entity_id: 0,
            unique_entity_id: 0,
            position: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            yaw: 0.0,
            pitch: 0.0,
            game_mode: GameMode::Survival,
            dimension: 0,
            spawn_position: None,
            health: 20.0,
            max_health: 20.0,
            hunger: 20.0,
            saturation: 5.0,
            xp_level: 0,
            xp_progress: 0.0,
            alive: true,
            deaths: 0,
            effects: Effects::default(),
            abilities: Abilities::default(),
            movement_speed: 0.1,
            sleep_flags: [false; 2],
            teleports: 0,
        }
    }
}

impl PlayerState {
    pub const PACKETS: &'static [u32] = &[
        StartGame::ID,
        MovePlayer::ID,
        CorrectPlayerMovePrediction::ID,
        UpdateAttributes::ID,
        SetHealth::ID,
        SetPlayerGameType::ID,
        UpdatePlayerGameType::ID,
        ChangeDimension::ID,
        SetSpawnPosition::ID,
        Respawn::ID,
        MobEffect::ID,
        UpdateAbilities::ID,
    ];

    /// Standing eye height of a player; server-sent player positions are offset by it.
    pub const EYE_HEIGHT: f32 = acacia_physics::constants::DEFAULT_PLAYER_HEIGHT_OFFSET;

    pub fn eye_position(&self) -> Vec3f {
        Vec3f { y: self.position.y + Self::EYE_HEIGHT, ..self.position.clone() }
    }

    /// Flight is allowed: by the may-fly ability, or in creative or spectator mode, where BDS 1.26.52's abilities
    /// layer lacks it (`PlayerAction` StartFlying's game mode check grants the flight).
    pub fn may_fly(&self) -> bool {
        self.abilities.may_fly || matches!(self.game_mode, GameMode::Creative | GameMode::Spectator | GameMode::CreativeSpectator)
    }

    /// In a bed, as the server shows it.
    pub fn is_sleeping(&self) -> bool {
        self.sleep_flags.contains(&true)
    }

    /// `SetEntityData` is routed here separately: it is not a movement packet (see `PACKETS`).
    pub(crate) fn apply_entity_data(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        // Every entity's metadata arrives here; peek the leading runtime id so others cost no decode.
        if acacia_client::proto::codec::read_varint64(&mut &packet.body[..])? != self.runtime_entity_id {
            return Ok(());
        }
        let p: SetEntityData = packet.decode()?;
        for item in p.metadata {
            match item.value {
                Meta::FlagsExtended(f) => self.sleep_flags[0] = f.contains(MetadataFlags2::SLEEPING),
                Meta::Default(MetadataDictionaryItemValueDefault::Byte(b)) if item.key == MetadataDictionaryItemKey::PlayerFlags => {
                    self.sleep_flags[1] = b & PLAYER_FLAG_SLEEP != 0;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            StartGame::ID => self.on_start_game(packet.decode()?),
            MovePlayer::ID => {
                let p: MovePlayer = packet.decode()?;
                if u64::from(p.runtime_id) == self.runtime_entity_id {
                    self.position = feet(p.position);
                    self.yaw = p.yaw;
                    self.pitch = p.pitch;
                    self.teleports += 1;
                }
            }
            CorrectPlayerMovePrediction::ID => {
                let p: CorrectPlayerMovePrediction = packet.decode()?;
                if p.prediction_type == CorrectPlayerMovePredictionPredictionType::Player {
                    self.position = feet(p.position);
                }
            }
            UpdateAttributes::ID => {
                let p: UpdateAttributes = packet.decode()?;
                if p.runtime_entity_id == self.runtime_entity_id {
                    p.attributes.iter().for_each(|a| self.on_attribute(a));
                }
            }
            MobEffect::ID => {
                let p: MobEffect = packet.decode()?;
                if p.runtime_entity_id == self.runtime_entity_id {
                    self.effects.apply(&p);
                }
            }
            UpdateAbilities::ID => {
                let p: UpdateAbilities = packet.decode()?;
                tracing::debug!(entity = p.entity_unique_id, me = self.unique_entity_id, layers = ?p.abilities, "abilities");
                if p.entity_unique_id == self.unique_entity_id {
                    self.abilities.apply(&p);
                }
            }
            SetHealth::ID => self.set_health(packet.decode::<SetHealth>()?.health as f32),
            SetPlayerGameType::ID => self.game_mode = packet.decode::<SetPlayerGameType>()?.gamemode,
            UpdatePlayerGameType::ID => {
                let p: UpdatePlayerGameType = packet.decode()?;
                if p.player_unique_id == self.unique_entity_id {
                    self.game_mode = p.gamemode;
                }
            }
            ChangeDimension::ID => {
                let p: ChangeDimension = packet.decode()?;
                self.dimension = p.dimension;
                self.position = feet(p.position);
            }
            SetSpawnPosition::ID => {
                let p: SetSpawnPosition = packet.decode()?;
                if p.spawn_type == SetSpawnPositionSpawnType::Player {
                    self.spawn_position = Some(p.player_position);
                }
            }
            Respawn::ID => {
                let p: Respawn = packet.decode()?;
                if p.state == RESPAWN_READY {
                    self.position = feet(p.position);
                    self.health = self.max_health;
                    self.alive = true;
                    self.sleep_flags = [false; 2];
                    self.effects.clear();
                    self.teleports += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn on_start_game(&mut self, p: StartGame) {
        self.unique_entity_id = p.entity_id;
        self.runtime_entity_id = p.runtime_entity_id;
        self.position = feet(p.player_position);
        // Vec2f is (pitch, yaw) here: gophertunnel's StartGame writes Pitch then Yaw.
        self.pitch = p.rotation.x;
        self.yaw = p.rotation.z;
        self.game_mode = match p.player_gamemode {
            GameMode::Fallback => p.world_gamemode,
            mode => mode,
        };
        self.dimension = p.dimension.to_raw() as i32;
        self.spawn_position = Some(p.spawn_position);
    }

    fn on_attribute(&mut self, a: &PlayerAttributesItem) {
        match a.name.as_str() {
            "minecraft:health" => {
                self.max_health = a.max;
                self.set_health(a.current);
            }
            "minecraft:movement" => self.movement_speed = a.current,
            "minecraft:player.hunger" => self.hunger = a.current,
            "minecraft:player.saturation" => self.saturation = a.current,
            "minecraft:player.level" => self.xp_level = a.current as i32,
            "minecraft:player.experience" => self.xp_progress = a.current,
            _ => {}
        }
    }

    fn set_health(&mut self, health: f32) {
        if (health > 0.0) != self.alive {
            tracing::debug!(health, "alive changed");
            self.deaths += u32::from(self.alive);
        }
        self.health = health;
        self.alive = health > 0.0;
    }
}

fn feet(eye: Vec3f) -> Vec3f {
    Vec3f { y: eye.y - PlayerState::EYE_HEIGHT, ..eye }
}

#[cfg(test)]
mod tests;
