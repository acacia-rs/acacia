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

    /// Biome tint averaged over the columns around `p` (the table's blend), so biome borders blend.
    fn tint(&self, [x, y, z]: [i32; 3], tint: Tint) -> [u8; 3] {
        let here = self.v.biome(x, y, z);
        let biome = self.biomes.get(here);
        let Some(color) = tint.color(biome) else { return [255; 3] };
        let reach = i32::from(self.table.biome_blend).min(volume::BLEND);
        let around = move || (-reach..=reach).flat_map(move |dx| (-reach..=reach).map(move |dz| (dx, dz)));
        let patched = tint == Tint::Grass && biome.grass_patch.is_some();
        let same = || !patched && around().all(|(dx, dz)| self.v.biome(x + dx, y, z + dz) == here);
        if !matches!(tint, Tint::Grass | Tint::Foliage | Tint::DryFoliage | Tint::Water) || same() {
            return color;
        }
        let [wx, _, wz] = self.world([x, y, z]);
        let mut sum = [0u32; 3];
        for (dx, dz) in around() {
            let biome = self.biomes.get(self.v.biome(x + dx, y, z + dz));
            let c = if tint == Tint::Grass { biome.grass_at(wx + dx, wz + dz) } else { tint.color(biome).unwrap_or(color) };
            (0..3).for_each(|i| sum[i] += u32::from(c[i]));
        }
        let count = (2 * reach as u32 + 1).pow(2);
        sum.map(|s| (s / count) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::BiomeTint;

    #[test]
    fn a_tint_blends_the_biomes_within_the_looks_reach() {
        let grey = |v: u8| BiomeTint { grass: [v; 3], grass_patch: None, foliage: [v; 3], dry_foliage: [v; 3], water: [v; 3] };
        let biomes = BiomeColors::of(&[(0, grey(0)), (1, grey(250))]);
        let mut v = Volume::filled(0);
        (8..18).for_each(|x| (-2..18).for_each(|z| v.biomes[volume::biome_cell(x, 0, z)] = 1));
        let tint = |reach: u8, x: i32, tint: Tint| {
            let mut table = BlockTable::from_blocks(Vec::new());
            table.biome_blend = reach;
            Ctx { v: &v, table: &table, biomes: &biomes }.tint([x, 0, 5], tint)
        };
        assert_eq!(tint(1, 7, Tint::Grass), [83; 3], "one column of three");
        assert_eq!(tint(2, 7, Tint::Grass), [100; 3], "two columns of five");
        assert_eq!(tint(2, 5, Tint::Water), [0; 3]);
        assert_eq!(tint(2, 15, Tint::Foliage), [250; 3], "the section's border has the next one's biomes");
        assert_eq!(tint(2, 7, Tint::Fixed([1, 2, 3])), [1, 2, 3]);
        assert_eq!(tint(2, 7, Tint::None), [255; 3]);
    }
}
