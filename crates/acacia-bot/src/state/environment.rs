//! World conditions the client is told about: time of day, weather and boss bars.

use std::collections::HashMap;

use acacia_client::proto::codec::read_zigzag32;
use acacia_client::proto::packets::{
    BiomeDefinitionList, BossEvent, BossEventColor, BossEventOverlay, BossEventType, ChangeDimension, LevelEvent, LevelEventEvent,
    SetTime, StartGame, SyncWorldClocks, SyncWorldClocksContent,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use acacia_world::biome_ids;

/// A boss bar shown to the player, keyed by the entity it belongs to in [`Environment::boss_bars`].
#[derive(Debug, Clone, PartialEq)]
pub struct BossBar {
    pub title: String,
    /// 0..1.
    pub progress: f32,
    pub color: BossEventColor,
    pub overlay: BossEventOverlay,
}

#[derive(Debug, Default)]
pub struct Environment {
    /// Time of day in ticks as last sent: `SetTime`, or the overworld clock of `SyncWorldClocks`
    /// (1.26 servers send only the latter). The client advances it locally in between.
    pub time: i32,
    /// The day cycle is stopped (`SyncWorldClocks` only).
    pub time_paused: bool,
    day_clock: Option<u64>,
    /// 0..1, ramping 0.01 a tick towards [`Environment::rain_target`] as BDS `Weather::tick` does.
    pub rain: f32,
    /// 1 from StartRain, 0 from StopRain: BDS's event data saturates, so it is no intensity
    /// (docs/research/riptide-rain.md).
    pub rain_target: f32,
    /// Server biome climates by chunk biome id (`BiomeDefinitionList`).
    pub biomes: HashMap<u32, BiomeClimate>,
    /// 0..1; above 0 during a thunderstorm.
    pub thunder: f32,
    pub boss_bars: HashMap<i64, BossBar>,
}

/// A biome's weather as the server defines it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiomeClimate {
    pub temperature: f32,
    /// Precipitation at all (`BiomeDefinition.rain`); snow instead of rain at a temperature of 0.15 or below.
    pub rain: bool,
}

const DAY_CLOCK: &str = "minecraft:overworld";
/// LevelEvent thunder intensity is 0..65535.
const LEVEL_EVENT_SCALE: f32 = 65535.0;
const RAIN_STEP: f32 = 0.01;
/// BDS `Weather::isPrecipitatingAt` needs the level strictly above this.
const RAIN_THRESHOLD: f32 = 0.2;
/// `Biome::getTemperature` above this rains; at or below it snows.
const SNOW_TEMPERATURE: f32 = 0.15;

impl Environment {
    pub const PACKETS: &'static [u32] =
        &[StartGame::ID, SetTime::ID, SyncWorldClocks::ID, LevelEvent::ID, BossEvent::ID, ChangeDimension::ID, BiomeDefinitionList::ID];

    /// The server last said it rains (the level may still be ramping).
    pub fn is_raining(&self) -> bool {
        self.rain_target > 0.0
    }

    /// One client tick of the rain level's ramp.
    pub fn tick(&mut self) {
        self.rain = if self.rain < self.rain_target {
            (self.rain + RAIN_STEP).min(self.rain_target)
        } else {
            (self.rain - RAIN_STEP).max(self.rain_target)
        };
    }

    /// Rain, not snow, falls in biome `biome` at the current level (sky exposure is the caller's).
    /// TODO: BDS lowers the temperature above y 80 and patches frozen oceans with noise (riptide-rain.md); the
    /// biome's base temperature is used everywhere.
    pub fn rains_in(&self, biome: u32) -> bool {
        self.rain > RAIN_THRESHOLD && self.biomes.get(&biome).is_some_and(|b| b.rain && b.temperature > SNOW_TEMPERATURE)
    }

    pub fn is_thundering(&self) -> bool {
        self.thunder > 0.0
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            StartGame::ID => {
                let p: StartGame = packet.decode()?;
                // The time follows in SetTime; StartGame only has the lock time of a stopped day cycle.
                (self.rain, self.thunder) = (p.rain_level, p.lightning_level);
                self.rain_target = if p.rain_level > 0.0 { 1.0 } else { 0.0 };
            }
            SetTime::ID => self.time = packet.decode::<SetTime>()?.time,
            SyncWorldClocks::ID => match packet.decode::<SyncWorldClocks>()?.content {
                SyncWorldClocksContent::InitializeRegistry(r) => {
                    if let Some(c) = r.clocks.iter().find(|c| c.name == DAY_CLOCK) {
                        (self.day_clock, self.time, self.time_paused) = (Some(c.id), c.time, c.paused);
                    }
                }
                SyncWorldClocksContent::SyncState(s) => {
                    // The first state can precede the registry that names the clocks.
                    let known = self.day_clock;
                    if let Some(c) = s.sync_states.iter().find(|c| known.is_none_or(|id| id == c.clock_id)) {
                        (self.time, self.time_paused) = (c.time, c.paused);
                    }
                }
                _ => {}
            },
            // Mostly sounds and particles: peek the event so those cost no decode.
            LevelEvent::ID => {
                let event = LevelEventEvent::from_raw(read_zigzag32(&mut &packet.body[..])?.into());
                if matches!(event, LevelEventEvent::StartRain | LevelEventEvent::StopRain | LevelEventEvent::StartThunder | LevelEventEvent::StopThunder) {
                    let level = packet.decode::<LevelEvent>()?.data as f32 / LEVEL_EVENT_SCALE;
                    match event {
                        LevelEventEvent::StartRain => self.rain_target = 1.0,
                        LevelEventEvent::StartThunder => self.thunder = level.max(f32::MIN_POSITIVE),
                        LevelEventEvent::StopRain => self.rain_target = 0.0,
                        _ => self.thunder = 0.0,
                    }
                }
            }
            BiomeDefinitionList::ID => self.biome_definitions(&packet.decode()?),
            BossEvent::ID => self.boss_event(packet.decode()?),
            // Boss bars belong to entities of the old dimension.
            ChangeDimension::ID => self.boss_bars.clear(),
            _ => {}
        }
        Ok(())
    }

    fn biome_definitions(&mut self, list: &BiomeDefinitionList) {
        for d in &list.biome_definitions {
            let Some(name) = usize::try_from(d.name_index).ok().and_then(|i| list.string_list.get(i)) else { continue };
            let vanilla = || biome_ids::vanilla(name.strip_prefix("minecraft:").unwrap_or(name));
            let id = if d.biome_id == biome_ids::UNSET { vanilla() } else { Some(d.biome_id) };
            if let Some(id) = id {
                self.biomes.insert(u32::from(id), BiomeClimate { temperature: d.temperature, rain: d.rain });
            }
        }
    }

    fn boss_event(&mut self, p: BossEvent) {
        let id = p.target_entity_id;
        match p.r#type {
            BossEventType::ShowBar => {
                self.boss_bars.insert(id, BossBar { title: p.title, progress: p.progress, color: p.color, overlay: p.overlay });
            }
            BossEventType::HideBar => {
                self.boss_bars.remove(&id);
            }
            BossEventType::SetBarProgress => {
                if let Some(b) = self.boss_bars.get_mut(&id) {
                    b.progress = p.progress;
                }
            }
            BossEventType::SetBarTitle => {
                if let Some(b) = self.boss_bars.get_mut(&id) {
                    b.title = p.title;
                }
            }
            BossEventType::UpdateProperties | BossEventType::Texture => {
                if let Some(b) = self.boss_bars.get_mut(&id) {
                    (b.color, b.overlay) = (p.color, p.overlay);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
