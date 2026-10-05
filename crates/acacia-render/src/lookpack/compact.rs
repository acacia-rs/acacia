//! Dropping what a pack no longer draws after blocks were replaced ([`LookPack::set_block`]).

use std::sync::Arc;

use rustc_hash::FxHashMap;

use super::LookPack;
use crate::blocks::placed::Random;
use crate::blocks::{ModelFace, RenderBlock, Shape};

impl LookPack {
    /// Drops blocks no state uses and texture layers no block draws, renumbering the rest. The
    /// texture array has a layer limit, which a look built over another's blocks would pass.
    pub fn compact(&mut self) {
        let mut kept: FxHashMap<u32, u32> = FxHashMap::default();
        let mut blocks = Vec::new();
        for index in self.states.values_mut() {
            let old = *index;
            *index = *kept.entry(old).or_insert_with(|| {
                blocks.push(self.blocks[old as usize].clone());
                blocks.len() as u32 - 1
            });
        }

        // Layer 0 is the missing texture, whatever draws it.
        let mut used = vec![false; self.atlas.layers.len()];
        used[0] = true;
        blocks.iter().for_each(|b| layers(b, &mut |layer| used[usize::from(layer)] = true));
        let mut next = 0;
        let renumbered: Vec<u16> = used.iter().map(|&u| if u { next += 1; next - 1 } else { 0 }).collect();
        blocks.iter_mut().for_each(|b| renumber(b, &renumbered));
        let mut layer = 0;
        self.atlas.layers.retain(|_| {
            layer += 1;
            used[layer - 1]
        });
        self.atlas.animations.retain(|a| used[usize::from(a.layer)]);
        self.atlas.animations.iter_mut().for_each(|a| a.layer = renumbered[usize::from(a.layer)]);
        self.blocks = blocks;
        self.distinct.clear();
    }
}

/// Every texture layer `block` or an alternative of it draws.
fn layers(block: &RenderBlock, each: &mut dyn FnMut(u16)) {
    block.textures.iter().for_each(|t| each(*t));
    if let Shape::Model(faces) = &block.shape {
        faces.iter().for_each(|f| each(f.texture));
    }
    match block.random.as_deref() {
        Some(Random::Whole(alternatives)) => alternatives.0.iter().for_each(|(_, b)| layers(b, each)),
        Some(Random::Parts(parts)) => parts.iter().flat_map(|p| &p.0).flat_map(|(_, faces)| &faces[..]).for_each(|f| each(f.texture)),
        None => {}
    }
}

fn renumber(block: &mut RenderBlock, to: &[u16]) {
    let moved = |faces: &Arc<[ModelFace]>| faces.iter().map(|f| ModelFace { texture: to[usize::from(f.texture)], ..f.clone() }).collect::<Arc<[_]>>();
    block.textures = block.textures.map(|t| to[usize::from(t)]);
    if let Shape::Model(faces) = &block.shape {
        block.shape = Shape::Model(moved(faces));
    }
    block.random = block.random.as_deref().map(|random| {
        Arc::new(match random {
            Random::Whole(alternatives) => Random::Whole(alternatives.map(|b| {
                let mut b = b.clone();
                renumber(&mut b, to);
                b
            })),
            Random::Parts(parts) => Random::Parts(parts.iter().map(|p| p.map(moved)).collect()),
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::flipbook::{Animation, Atlas};
    use crate::assets::image::{TEXEL_BYTES, Texture};
    use crate::blocks::placed::Weighted;
    use crate::blocks::{BlockTable, Material, Tint};
    use crate::look::Look;

    #[test]
    fn replaced_blocks_and_their_textures_go() {
        let grey = |v: u8| Texture { rgba: Box::new([v; TEXEL_BYTES]) };
        let face = ModelFace { corners: [[0.0; 3]; 3], uv: [[0.0; 2]; 3], texture: 3, tint: Tint::None, material: Material::Opaque, shade: None, ambient_occlusion: false, cull: None };
        let model = RenderBlock { shape: Shape::Model([face].into()), ..BlockTable::cube(0) };
        let either = Random::Whole(Weighted(Box::new([(1, BlockTable::cube(2)), (1, BlockTable::cube(4))])));
        let animation = |layer| Animation { layer, frames: vec![grey(1), grey(2)], ticks_per_frame: 1, blend: false };
        let mut pack = LookPack {
            look: Look::JAVA,
            blocks: Vec::new(),
            states: Default::default(),
            distinct: Default::default(),
            atlas: Atlas { layers: (0..5).map(|i| grey(i * 10)).collect(), animations: vec![animation(1), animation(3)] },
            files: Default::default(),
        };
        pack.set_block("minecraft:stone".into(), BlockTable::cube(1));
        pack.set_block("minecraft:dirt".into(), RenderBlock { random: Some(Arc::new(either)), ..BlockTable::cube(2) });
        pack.set_block("minecraft:stone".into(), model);
        pack.compact();

        assert_eq!(pack.counts(), (2, 2));
        assert_eq!(pack.atlas.layers.iter().map(|t| t.rgba[0]).collect::<Vec<_>>(), [0, 20, 30, 40], "layer 1 was only the replaced stone's");
        let dirt = pack.block("minecraft:dirt").unwrap();
        assert_eq!(dirt.textures, [1; 6]);
        let Some(Random::Whole(alternatives)) = dirt.random.as_deref() else { panic!("dirt has alternatives") };
        assert_eq!(alternatives.0.iter().map(|(_, b)| b.textures[0]).collect::<Vec<_>>(), [1, 3]);
        let Shape::Model(faces) = &pack.block("minecraft:stone").unwrap().shape else { panic!("stone is a model") };
        assert_eq!(faces[0].texture, 2);
        assert_eq!(pack.atlas.animations.iter().map(|a| a.layer).collect::<Vec<_>>(), [2]);
    }
}
