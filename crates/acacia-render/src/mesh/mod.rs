//! Section meshing: a [`Volume`] (section plus border) becomes packed quads for the solid and
//! translucent passes.

mod greedy;
pub mod quad;
mod shapes;
pub mod visibility;
pub mod volume;

pub use quad::Quad;
pub use volume::Volume;

use crate::biome::BiomeColors;
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
    out
}

struct Ctx<'a> {
    v: &'a Volume,
    table: &'a BlockTable,
    biomes: &'a BiomeColors,
}

impl Ctx<'_> {
    fn block(&self, p: [i32; 3]) -> &RenderBlock {
        self.table.get(self.v.block(p[0], p[1], p[2]))
    }

    /// Face `face` of `b` at section-local `p`.
    fn surface(&self, p: [i32; 3], b: &RenderBlock, face: usize) -> Surface {
        let tint = b.tint[face];
        Surface { texture: b.textures[face], tint_kind: tint.shader_kind(), material: b.material[face], color: quantize(self.tint(p, tint)) }
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
