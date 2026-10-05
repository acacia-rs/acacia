//! Section meshing: a [`Volume`] (section plus border) becomes packed quads for the solid and
//! translucent passes.

mod greedy;
mod liquid;
pub mod model;
pub mod quad;
mod shapes;
pub mod visibility;
pub mod volume;

pub use quad::Quad;
pub use volume::Volume;

use std::sync::Arc;

use crate::biome::BiomeColors;
use crate::blocks::model::BlockModel;
use crate::blocks::placed::{self, Draw, Random};
use crate::blocks::{BlockTable, RenderBlock, Tint};
use crate::light::LightVolume;
use quad::{Surface, quantize};

#[derive(Default)]
pub struct SectionMesh {
    pub solid: Vec<Quad>,
    pub translucent: Vec<Quad>,
    /// Gathered with the mesh by the workers; `None` for empty meshes.
    pub light: Option<LightVolume>,
    /// Face pairs that see each other through the section ([`visibility`]).
    pub visibility: u16,
    /// Blocks drawn as entity models, by section-local position.
    pub models: Vec<([u8; 3], Arc<BlockModel>)>,
}

impl SectionMesh {
    pub fn is_empty(&self) -> bool {
        self.solid.is_empty() && self.translucent.is_empty()
    }
}

pub fn mesh_section(volume: &Volume, table: &BlockTable, biomes: &BiomeColors) -> SectionMesh {
    let ctx = Ctx { v: volume, table, biomes };
    let mut out = SectionMesh { visibility: visibility::section_visibility(volume, table), ..Default::default() };
    greedy::cubes(&ctx, &mut out);
    shapes::others(&ctx, &mut out);
    model::sort_for_blending(&mut out.translucent);
    out
}

struct Ctx<'a> {
    v: &'a Volume,
    table: &'a BlockTable,
    biomes: &'a BiomeColors,
}

impl Ctx<'_> {
    /// What the state at `p` is to its neighbours; what it draws there is [`Ctx::drawn`] of it.
    fn block(&self, p: [i32; 3]) -> &RenderBlock {
        self.table.get(self.v.block(p[0], p[1], p[2]))
    }

    fn world(&self, p: [i32; 3]) -> [i32; 3] {
        [0, 1, 2].map(|i| self.v.origin[i] + p[i])
    }

    fn draw(&self, p: [i32; 3]) -> Draw {
        Draw::new(placed::seed(self.world(p)))
    }

    /// The alternative of `b` that section-local `p` draws.
    fn drawn<'a>(&self, p: [i32; 3], b: &'a RenderBlock) -> &'a RenderBlock {
        match b.random.as_deref() {
            Some(Random::Whole(alternatives)) => alternatives.pick(self.draw(p)).unwrap_or(b),
            _ => b,
        }
    }

    /// Face `face` of `b` at section-local `p`.
    fn surface(&self, p: [i32; 3], b: &RenderBlock, face: usize) -> Surface {
        let tint = b.tint[face];
        Surface {
            texture: b.textures[face],
            tint_kind: tint.shader_kind(),
            material: b.material[face],
            color: quantize(self.tint(p, tint)),
            turn: b.turns[face],
        }
    }

    /// Biome tint averaged over the 3×3 columns around `p`, so biome borders blend.
    fn tint(&self, [x, y, z]: [i32; 3], tint: Tint) -> [u8; 3] {
        let mut sum = [0u32; 3];
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(c) = tint.color(self.biomes.get(self.v.biome(x + dx, y, z + dz))) else { return [255; 3] };
                (0..3).for_each(|i| sum[i] += u32::from(c[i]));
            }
        }
        sum.map(|s| (s / 9) as u8)
    }
}
