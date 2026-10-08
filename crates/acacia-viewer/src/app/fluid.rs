//! What the camera is in, for the renderer's fluid fog (Java's `Camera.getFluidInCamera`).

use acacia_render::fluid_view::{Fluid, InFluid};
use acacia_world::World;
use glam::DVec3;

use super::App;

impl App {
    /// Tells the renderer the fluid at the camera and how long it has been in it.
    pub(super) fn feed_fluid(&mut self, dt: f32) {
        let Some(r) = &mut self.renderer else { return };
        let found = r.world().and_then(|w| fluid_at(w, self.camera.position));
        let seconds = match (r.in_fluid, found) {
            (Some(was), Some((fluid, _))) if was.fluid == fluid => was.seconds + dt,
            _ => 0.0,
        };
        r.in_fluid = found.map(|(fluid, biome)| InFluid { fluid, biome, seconds });
    }
}

/// The fluid at `eye`, and the biome there. Powder snow fills its block; a liquid reaches its
/// surface height, or the whole block under more of the same.
pub fn fluid_at(world: &World, eye: DVec3) -> Option<(Fluid, Option<u32>)> {
    let p = eye.floor().as_ivec3();
    let chunk = world.get(p.x >> 4, p.z >> 4)?;
    let chunk = chunk.read();
    let registry = world.registry();
    // A waterlogged block keeps its water in the second layer.
    let liquid = |y: i32| [chunk.block(p.x, y, p.z), chunk.liquid(p.x, y, p.z)].into_iter().filter_map(|id| registry.get(id)).find(|s| s.is_liquid());
    let fluid = match registry.get(chunk.block(p.x, p.y, p.z))?.name {
        "minecraft:powder_snow" => Fluid::PowderSnow,
        _ => {
            let state = liquid(p.y)?;
            let covered = liquid(p.y + 1).is_some_and(|above| above.is_lava() == state.is_lava());
            let surface = f64::from(p.y) + if covered { 1.0 } else { f64::from(state.fluid_height()) };
            if eye.y >= surface {
                return None;
            }
            if state.is_lava() { Fluid::Lava } else { Fluid::Water }
        }
    };
    Some((fluid, chunk.biome(p.x, p.y, p.z)))
}

#[cfg(test)]
mod tests {
    use acacia_world::{BlockIds, BlockRegistry, ChunkView};

    use super::*;

    #[test]
    fn the_eye_is_in_a_liquid_below_its_surface() {
        let registry = BlockRegistry::vanilla();
        let mut view = ChunkView::new(World::new(BlockRegistry::vanilla_arc(), 0, BlockIds::Runtime));
        // One version-9 section at y 64..80, all air (its id zigzag-varint encoded).
        let mut section = vec![9, 1, 4, 1];
        let mut v = registry.air_id() << 1;
        while v >= 0x80 {
            section.push(v as u8 | 0x80);
            v >>= 7;
        }
        section.push(v as u8);
        view.insert_level_chunk(0, 0, 1, &section).unwrap();
        let world = view.world().clone();
        let place = |y, name, props| assert!(world.set_block(1, y, 1, 0, registry.find(name, props).unwrap()));
        place(64, "minecraft:water", "liquid_depth=0");
        place(70, "minecraft:lava", "liquid_depth=0");
        place(71, "minecraft:lava", "liquid_depth=0");
        place(75, "minecraft:powder_snow", "");
        let at = |y| fluid_at(&world, DVec3::new(1.5, y, 1.5)).map(|(f, _)| f);
        // A source's surface is 8/9 of the block up.
        assert_eq!((at(64.5), at(64.95), at(65.5)), (Some(Fluid::Water), None, None));
        assert_eq!((at(70.95), at(75.9)), (Some(Fluid::Lava), Some(Fluid::PowderSnow)));
    }
}
