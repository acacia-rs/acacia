//! [`WeatherLevel`] and [`SplashLevel`] over a Bedrock world, with the biomes' Java climate.

use acacia_world::{Aabb, BlockState, World};

use super::climate::{Precipitation, SEA_LEVEL};
use super::columns::WeatherLevel;
use super::splash::SplashLevel;
use crate::biome::BiomeColors;

pub struct WorldWeather<'a> {
    pub world: &'a World,
    pub biomes: &'a BiomeColors,
    /// `[block, sky]` light at a block.
    pub light: &'a dyn Fn([i32; 3]) -> [u8; 2],
}

/// Java's `isSolid` (`calculateSolid`): a collision shape whose bounds average at least 0.729 of
/// a block a side, or are a block tall.
fn solid(state: &BlockState) -> bool {
    let Some(first) = state.boxes.first() else { return false };
    let (min, max) = state.boxes.iter().fold((first.min, first.max), |(lo, hi), b| (std::array::from_fn(|i| lo[i].min(b.min[i])), std::array::from_fn(|i| hi[i].max(b.max[i]))));
    let size: [f32; 3] = std::array::from_fn(|i| max[i] - min[i]);
    f64::from(size.iter().sum::<f32>()) / 3.0 >= 0.729_166_666_666_666_6 || size[1] >= 1.0
}

/// `Heightmap.Types.MOTION_BLOCKING`: blocks motion (solid, not cobweb or a bamboo shoot) or
/// holds a fluid.
pub fn blocks_motion(state: &BlockState, liquid: &BlockState) -> bool {
    let name = state.name.strip_prefix("minecraft:").unwrap_or(state.name);
    state.is_liquid() || liquid.is_liquid() || (solid(state) && !matches!(name, "web" | "cobweb" | "bamboo_sapling"))
}

impl WorldWeather<'_> {
    fn states(&self, [x, y, z]: [i32; 3]) -> Option<(&BlockState, &BlockState)> {
        let chunk = self.world.get(x >> 4, z >> 4)?;
        let chunk = chunk.read();
        let registry = self.world.registry();
        Some((registry.get(chunk.block(x, y, z))?, registry.get(chunk.liquid(x, y, z))?))
    }
}

impl WeatherLevel for WorldWeather<'_> {
    fn surface(&self, x: i32, z: i32) -> i32 {
        let dimension = self.world.dimension();
        let Some(chunk) = self.world.get(x >> 4, z >> 4) else { return dimension.min_y };
        let chunk = chunk.read();
        let registry = self.world.registry();
        for section in (0..chunk.section_count()).rev() {
            if chunk.section_uniform(section) == Some(dimension.air) {
                continue;
            }
            let base = dimension.min_y + section as i32 * 16;
            for y in (base..base + 16).rev() {
                let (Some(block), Some(liquid)) = (registry.get(chunk.block(x, y, z)), registry.get(chunk.liquid(x, y, z))) else { continue };
                if blocks_motion(block, liquid) {
                    return y + 1;
                }
            }
        }
        dimension.min_y
    }

    fn precipitation(&self, [x, y, z]: [i32; 3]) -> Precipitation {
        let Some(chunk) = self.world.get(x >> 4, z >> 4) else { return Precipitation::None };
        let biome = chunk.read().biome(x, y, z);
        biome.map_or(crate::biome::PLAINS_CLIMATE, |id| self.biomes.climate(id)).precipitation_at([x, y, z], SEA_LEVEL)
    }

    fn light(&self, p: [i32; 3]) -> [u8; 2] {
        (self.light)(p)
    }
}

impl SplashLevel for WorldWeather<'_> {
    fn min_y(&self) -> i32 {
        self.world.dimension().min_y
    }

    fn splash_top(&self, p: [i32; 3], x: f64, z: f64) -> f64 {
        let Some((block, liquid)) = self.states(p) else { return 0.0 };
        let inside = |b: &&Aabb| (f64::from(b.min[0])..=f64::from(b.max[0])).contains(&x) && (f64::from(b.min[2])..=f64::from(b.max[2])).contains(&z);
        let top = block.boxes.iter().filter(inside).map(|b| f64::from(b.max[1])).fold(0.0, f64::max);
        let fluid = if block.is_liquid() { block } else { liquid };
        // FluidState.getHeight: a full block under more of the same fluid.
        let same = |s: &BlockState| (fluid.is_water() && s.is_water()) || (fluid.is_lava() && s.is_lava());
        let covered = self.states([p[0], p[1] + 1, p[2]]).is_some_and(|(b, l)| same(b) || same(l));
        top.max(if covered { 1.0 } else { f64::from(fluid.fluid_height()) })
    }

    fn smokes(&self, p: [i32; 3]) -> bool {
        let Some((block, liquid)) = self.states(p) else { return false };
        let name = block.name.strip_prefix("minecraft:").unwrap_or(block.name);
        block.is_lava() || liquid.is_lava() || name == "magma" || name == "magma_block" || (name.ends_with("campfire") && block.property("extinguished") != Some("true") && block.property("extinguished") != Some("1"))
    }
}
