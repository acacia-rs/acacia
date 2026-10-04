//! World conditions the client is told about: time of day, weather and boss bars.

use std::collections::HashMap;

use acacia_client::proto::codec::read_zigzag32;
use acacia_client::proto::packets::{
    BossEvent, BossEventColor, BossEventOverlay, BossEventType, ChangeDimension, LevelEvent, LevelEventEvent, SetTime, StartGame,
    SyncWorldClocks, SyncWorldClocksContent,
};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

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
    /// 0..1; above 0 while it rains.
    pub rain: f32,
    /// 0..1; above 0 during a thunderstorm.
    pub thunder: f32,
    pub boss_bars: HashMap<i64, BossBar>,
}

const DAY_CLOCK: &str = "minecraft:overworld";
/// LevelEvent rain/thunder intensity is 0..65535.
const LEVEL_EVENT_SCALE: f32 = 65535.0;

impl Environment {
    pub const PACKETS: &'static [u32] = &[StartGame::ID, SetTime::ID, SyncWorldClocks::ID, LevelEvent::ID, BossEvent::ID, ChangeDimension::ID];

    pub fn is_raining(&self) -> bool {
        self.rain > 0.0
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
                        LevelEventEvent::StartRain => self.rain = level.max(f32::MIN_POSITIVE),
                        LevelEventEvent::StartThunder => self.thunder = level.max(f32::MIN_POSITIVE),
                        LevelEventEvent::StopRain => self.rain = 0.0,
                        _ => self.thunder = 0.0,
                    }
                }
            }
            BossEvent::ID => self.boss_event(packet.decode()?),
            // Boss bars belong to entities of the old dimension.
            ChangeDimension::ID => self.boss_bars.clear(),
            _ => {}
        }
        Ok(())
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
