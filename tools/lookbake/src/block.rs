//! Baked Java faces to what the renderer draws: a cube it can merge with its neighbours when the
//! faces are a full block's, else the faces themselves.

use std::sync::Arc;

use acacia_render::assets::flipbook::Atlas;
use acacia_render::assets::image::Alpha;
use acacia_render::blocks::{Fluid, Layer, Material, ModelFace, RenderBlock, Shape, Tint};
use acacia_render::mesh::quad::turned;

use crate::model::bake::BakedFace;
use crate::textures::Textures;

const EPSILON: f32 = 1e-3;

/// `tint` is the colour of a tint index. `None` when a face's texture has no image.
pub fn model_faces(
    faces: &[BakedFace],
    tint: &dyn Fn(i32) -> Tint,
    ambient_occlusion: bool,
    textures: &mut Textures,
    atlas: &mut Atlas,
) -> Option<Vec<ModelFace>> {
    let face = |f: &BakedFace| {
        let (texture, alpha) = textures.layer(&f.texture, atlas)?;
        let material = match alpha {
            Alpha::Opaque => Material::Opaque,
            Alpha::Cutout => Material::Cutout,
            Alpha::Blended => Material::Blend,
        };
        Some(ModelFace {
            corners: [f.positions[0], f.positions[1], f.positions[3]],
            uv: [f.uvs[0], f.uvs[1], f.uvs[3]],
            texture,
            tint: f.tint_index.map_or(Tint::None, tint),
            material,
            shade: f.shade.map(|d| d.face()),
            ambient_occlusion,
            cull: f.cull.map(|d| d.face()),
        })
    };
    faces.iter().map(face).collect()
}

/// `model` is [`model_faces`] of `faces`. A full block's six faces make a cube when `cube` allows.
pub fn render_block(base: &RenderBlock, faces: &[BakedFace], model: Vec<ModelFace>, cube: bool) -> RenderBlock {
    let translucent = model.iter().any(|f| f.material == Material::Blend);
    let opaque = model.iter().all(|f| f.material == Material::Opaque);
    let full = (0..6).all(|side| faces.iter().any(|f| f.cull.map(|d| d.face()) == Some(side) && covers_side(f)));
    let mut block = RenderBlock {
        shape: Shape::None,
        layer: if translucent { Layer::Translucent } else { Layer::Solid },
        textures: [0; 6],
        tint: [Tint::None; 6],
        material: [Material::Opaque; 6],
        turns: [0; 6],
        random: None,
        offset: None,
        occludes: full && opaque,
        shades: full && opaque,
        cull_same: base.cull_same,
        fluid: Fluid::None,
        fluid_height: 0,
        model: None,
    };
    match side_turns(faces).filter(|_| cube && full && faces.len() == 6) {
        Some(turns) => {
            for (f, baked) in model.iter().zip(faces) {
                let side = usize::from(baked.cull.expect("a full block's faces cull").face());
                (block.textures[side], block.tint[side], block.material[side]) = (f.texture, f.tint, f.material);
            }
            (block.shape, block.turns) = (Shape::Cube, turns);
        }
        None => block.shape = Shape::Model(Arc::from(model)),
    }
    block
}

/// Two faces on the same quad facing the same way (grass block sides under their overlay): they
/// would fight for depth. The two sides of a plane share corners but face opposite ways.
pub fn overlaid(faces: &[BakedFace]) -> bool {
    let corners = |f: &BakedFace| {
        let fixed = f.positions.map(|p| p.map(|v| (v * 64.0).round() as i32));
        let (a, b, c) = (fixed[0], fixed[1], fixed[3]);
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let normal = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]].map(i32::signum);
        let mut c = fixed;
        c.sort_unstable();
        (c, normal)
    };
    let mut seen: Vec<_> = faces.iter().map(corners).collect();
    seen.sort_unstable();
    seen.windows(2).any(|pair| pair[0] == pair[1])
}

/// How the texture of each side's first full face lies, when every side has one that lies as a
/// cube's can.
pub fn side_turns(faces: &[BakedFace]) -> Option<[u8; 6]> {
    let mut turns = [0; 6];
    for (side, turn) in turns.iter_mut().enumerate() {
        *turn = turn_of(faces.iter().find(|f| f.cull.map(|d| usize::from(d.face())) == Some(side) && covers_side(f))?)?;
    }
    Some(turns)
}

/// The face fills its side of the block.
fn covers_side(f: &BakedFace) -> bool {
    let on_corner = |v: f32| v.abs() < EPSILON || (v - 16.0).abs() < EPSILON;
    f.positions.iter().all(|p| p.iter().all(|v| on_corner(*v))) && f.shade == f.cull
}

/// The turn that takes the texture a cube face has by position to this face's.
fn turn_of(f: &BakedFace) -> Option<u8> {
    let side = f.cull?;
    (0..8).find(|turn| {
        f.positions.iter().zip(&f.uvs).all(|(p, uv)| {
            let [u, v] = turned(*turn, side.project(*p));
            (u - uv[0]).abs() < EPSILON && (v - uv[1]).abs() < EPSILON
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::bake::Direction;

    fn face(positions: [[f32; 3]; 4]) -> BakedFace {
        let uvs = positions.map(|p| Direction::South.project(p));
        BakedFace { positions, uvs, texture: "block/poppy".into(), cull: None, tint_index: None, shade: None, ambient_occlusion: true }
    }

    #[test]
    fn a_plane_drawn_from_both_sides_is_not_overlaid() {
        let front = face([[0.0, 16.0, 8.0], [0.0, 0.0, 8.0], [16.0, 0.0, 8.0], [16.0, 16.0, 8.0]]);
        let back = face([[16.0, 16.0, 8.0], [16.0, 0.0, 8.0], [0.0, 0.0, 8.0], [0.0, 16.0, 8.0]]);
        assert!(!overlaid(&[front.clone(), back]));
        assert!(overlaid(&[front.clone(), front]));
    }

    #[test]
    fn a_full_face_knows_how_its_texture_is_turned() {
        let south = [[0.0, 16.0, 16.0], [0.0, 0.0, 16.0], [16.0, 0.0, 16.0], [16.0, 16.0, 16.0]];
        let turn = |uvs: [[f32; 2]; 4]| {
            turn_of(&BakedFace { uvs, cull: Some(Direction::South), shade: Some(Direction::South), ..face(south) })
        };
        let plain = south.map(|p| Direction::South.project(p));
        assert_eq!(turn(plain), Some(0));
        assert_eq!(turn(plain.map(|[u, v]| [16.0 - u, v])), Some(4));
        assert_eq!(turn(plain.map(|[u, v]| [16.0 - u, 16.0 - v])), Some(2));
        assert_eq!(turn(plain.map(|[u, v]| [u / 2.0, v])), None);
    }
}
