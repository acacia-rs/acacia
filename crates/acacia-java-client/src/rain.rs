//! `Level.isRainingAt` on the client: the weather the server sends, sky light (`sky_light.rs`, relit after block
//! changes by `relight.rs`), the MOTION_BLOCKING heightmap and the biome's precipitation there.

use acacia_java_physics::fluid_of;
use acacia_java_proto::light::{read_sky_light, skip_block_entities};
use acacia_java_proto::{Error, Wire, ids, read_varint};
use acacia_registry::climate::{Climate, Precipitation};
use acacia_registry::{BlockState, BlockTag};

use crate::biomes::noise_cell;
use crate::join::Client;
use crate::world::World;

use ids::play::clientbound as from;

/// `ClientboundGameEventPacket`'s weather events.
const START_RAINING: u8 = 1;
const STOP_RAINING: u8 = 2;
const RAIN_LEVEL_CHANGE: u8 = 7;

/// What rain needs beyond the blocks.
#[derive(Debug, Default)]
pub struct Weather {
    /// `Level.rainLevel`; the client sets it as sent and never ramps it itself.
    pub rain_level: f32,
    /// `CommonPlayerSpawnInfo.seed`: the hashed seed `BiomeManager` zooms with.
    pub(crate) zoom_seed: i64,
    pub(crate) sea_level: i32,
    /// `worldgen/biome` climates by network id, from configuration.
    pub(crate) climates: Vec<Option<Climate>>,
}

/// `CommonPlayerSpawnInfo` after the dimension: the zoom seed and sea level.
pub(crate) fn read_spawn_rest(input: &mut &[u8]) -> Result<(i64, i32), Error> {
    let seed = i64::read(input)?;
    read_varint(input)?;
    read_varint(input)?;
    bool::read(input)?;
    bool::read(input)?;
    if bool::read(input)? {
        String::read(input)?;
        i64::read(input)?;
    }
    read_varint(input)?;
    Ok((seed, read_varint(input)?))
}

/// `Heightmap.Types.MOTION_BLOCKING`.
fn motion_blocking(state: BlockState) -> bool {
    state.block().is(BlockTag::BlocksMotionInHeightmap) || fluid_of(state).is_some()
}

impl World {
    /// What follows a chunk's sections: its block entities, then its light.
    pub(crate) fn read_chunk_light(&mut self, chunk: (i32, i32), input: &mut &[u8]) -> Result<(), Error> {
        skip_block_entities(input)?;
        self.sky.queue(chunk, read_sky_light(input)?);
        Ok(())
    }

    /// Weather events and `LightUpdate`.
    pub(crate) fn apply_weather(&mut self, id: i32, input: &mut &[u8]) -> Result<(), Error> {
        match id {
            from::GAME_EVENT => {
                let (event, value) = (u8::read(input)?, f32::read(input)?);
                // `setRainLevel` clamps; starting sets 0 and stopping 1, and level changes follow.
                match event {
                    START_RAINING => self.weather.rain_level = 0.0,
                    STOP_RAINING => self.weather.rain_level = 1.0,
                    RAIN_LEVEL_CHANGE => self.weather.rain_level = value.clamp(0.0, 1.0),
                    _ => {}
                }
            }
            from::LIGHT_UPDATE => {
                let at = (read_varint(input)?, read_varint(input)?);
                self.sky.queue(at, read_sky_light(input)?);
            }
            _ => {}
        }
        Ok(())
    }

    /// `Level.isRaining`: rain above 0.2 in a dimension with weather.
    pub fn is_raining(&self) -> bool {
        let weather = !matches!(self.dimension.as_str(), "minecraft:the_nether" | "minecraft:the_end");
        weather && f64::from(self.weather.rain_level) > 0.2
    }

    /// `Level.isRainingAt`.
    pub fn is_raining_at(&self, pos: [i32; 3]) -> bool {
        if !self.is_raining() || !self.sky.sees_sky(pos, self.min_y()) {
            return false;
        }
        let top = self.chunks().max_y(pos[0], pos[2]).unwrap_or(pos[1]);
        if (pos[1]..top).any(|y| self.block([pos[0], y, pos[2]]).and_then(BlockState::from_id).is_some_and(motion_blocking)) {
            return false;
        }
        // An unloaded column reads plains, as `ClientLevel.getUncachedNoiseBiome` does.
        let climate = match self.chunks().noise_biome(noise_cell(self.weather.zoom_seed, pos)) {
            Some(id) => self.weather.climates.get(id as usize).copied().flatten(),
            None => Climate::vanilla("plains"),
        };
        climate.is_some_and(|c| c.precipitation_at(pos, self.weather.sea_level) == Precipitation::Rain)
    }
}

#[cfg(test)]
mod tests {
    use acacia_java_proto::chunk::{Section, Values};
    use acacia_registry::Block;

    use super::*;

    #[test]
    fn it_rains_on_open_plains_but_not_under_glass_or_in_a_desert() {
        let mut world = World { dimension: "minecraft:overworld".into(), ..World::default() };
        let air = Section { non_empty: 0, fluids: 0, blocks: Values::Single(0), biomes: Values::Single(0) };
        let mut column = vec![air; 24];
        column[5].blocks = Values::Single(Block::Glass.default_state().id());
        world.chunks.insert(0, 0, column.clone());
        column[5].blocks = Values::Single(0);
        column.iter_mut().for_each(|s| s.biomes = Values::Single(1));
        world.chunks.insert(1, 0, column);
        world.weather = Weather { rain_level: 1.0, sea_level: 63, climates: vec![Climate::vanilla("plains"), Climate::vanilla("desert")], ..Weather::default() };
        assert!(world.is_raining_at([3, 100, 3]), "above the glass");
        assert!(!world.is_raining_at([3, -40, 3]), "under the glass at y 16..31");
        assert!(!world.is_raining_at([20, 64, 3]), "desert");
        world.weather.rain_level = 0.19;
        assert!(!world.is_raining_at([3, 100, 3]), "not raining yet");
    }
}

impl Client {
    /// `Entity.isInRain`: rain at the feet' block or at the top of the box above it.
    pub fn in_rain(&self) -> bool {
        let player = &self.movement.player;
        let feet = player.pos.map(|v| v.floor() as i32);
        let head = player.bounding_box().max[1].floor() as i32;
        self.world.is_raining_at(feet) || self.world.is_raining_at([feet[0], head, feet[2]])
    }
}
