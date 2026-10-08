//! A block item: the block's own faces, from the look pack's texture array, as an entity mesh
//! with its own texture. Tints and materials are baked into the texels; see README "Items".

use glam::{Mat4, Vec3};

use crate::assets::flipbook::Atlas;
use crate::assets::image::{TEXEL_BYTES, Texture};
use crate::biome::PLAINS;
use crate::blocks::{Box16, Material, RenderBlock, Shape, Tint};
use crate::entity::Skin;
use crate::entity::bake::{Joint, Mesh, Vertex};

const TILE: u32 = 16;

/// A quad in block space (0..1, Java axes) with UVs inside its tile.
pub(super) struct Face {
    pub corners: [Vec3; 4],
    pub uv: [[f32; 2]; 4],
    pub tile: Tile,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Tile {
    pub layer: u16,
    tint: Tint,
    material: Material,
}

/// The block drawn as an item, or `None` for shapes that are not boxes or model faces.
pub fn skin(block: &RenderBlock, atlas: &Atlas) -> Option<Skin> {
    let faces = faces(block)?;
    let mut tiles: Vec<Tile> = Vec::new();
    for face in &faces {
        if !tiles.contains(&face.tile) {
            tiles.push(face.tile);
        }
    }
    let rows = tiles.len() as f32;
    let mut vertices = Vec::with_capacity(faces.len() * 6);
    for face in &faces {
        let row = tiles.iter().position(|t| *t == face.tile).unwrap_or(0) as f32;
        let [a, b, _, d] = face.corners;
        let n = (b - a).cross(d - a).normalize_or_zero();
        // Block space to model space: centred, and z mirrored (models face -z).
        let at = |p: Vec3| [p.x - 0.5, p.y - 0.5, 0.5 - p.z];
        let normal = [n.x, n.y, -n.z];
        let vertex = |i: usize| {
            let [u, v] = face.uv[i];
            Vertex { position: at(face.corners[i]), bone: 0, normal, uv: [u.clamp(0.0, 1.0), (row + v.clamp(0.0, 1.0)) / rows] }
        };
        vertices.extend([0, 1, 2, 0, 2, 3].map(vertex));
    }
    let rgba = tiles.iter().flat_map(|t| texels(atlas.layers.get(t.layer as usize), *t)).collect();
    let joint = Joint { parent: None, pivot: Vec3::ZERO, rotation: [0.0; 3], unbind: Mat4::IDENTITY };
    let mesh = Mesh { vertices, bones: vec!["root".into()], joints: vec![joint] };
    Some(Skin { width: TILE, height: TILE * tiles.len() as u32, rgba, mesh: Some(mesh) })
}

/// The block's faces, or `None` for shapes that are not boxes or model faces.
pub(super) fn faces(block: &RenderBlock) -> Option<Vec<Face>> {
    let tile = |face| tile_of(block, face);
    let faces: Vec<Face> = match &block.shape {
        Shape::Cube => box_faces([0, 0, 0, 16, 16, 16], tile).collect(),
        Shape::Boxes(boxes) => boxes.iter().flat_map(|b| box_faces(*b, tile)).collect(),
        Shape::Model(faces) => faces
            .iter()
            .map(|f| {
                let [c0, c1, c3] = f.corners.map(|c| Vec3::from(c) / 16.0);
                let [t0, t1, t3] = f.uv.map(|[u, v]| [u / 16.0, v / 16.0]);
                let t2 = [t1[0] + t3[0] - t0[0], t1[1] + t3[1] - t0[1]];
                let tile = Tile { layer: f.texture, tint: f.tint, material: f.material };
                Face { corners: [c0, c1, c1 + c3 - c0, c3], uv: [t0, t1, t2, t3], tile }
            })
            .collect(),
        _ => return None,
    };
    (!faces.is_empty()).then_some(faces)
}

/// A tile as the terrain would draw it, under plains colours: the entity pass knows no tints.
pub(super) fn texels(texture: Option<&Texture>, tile: Tile) -> Vec<u8> {
    let mut rgba = texture.map_or_else(|| vec![255; TEXEL_BYTES], |t| t.rgba.to_vec());
    let colour = tile.tint.color(&PLAINS);
    for px in rgba.chunks_exact_mut(4) {
        // Overlay alpha is the tint mask; the face itself is opaque.
        let tinted = colour.filter(|_| tile.material != Material::Overlay || px[3] == 255);
        if let Some(c) = tinted {
            (0..3).for_each(|i| px[i] = (u16::from(px[i]) * u16::from(c[i]) / 255) as u8);
        }
        px[3] = match tile.material {
            Material::Opaque | Material::Overlay => 255,
            Material::Cutout => if px[3] < 128 { 0 } else { 255 },
            Material::Blend => px[3],
        };
    }
    rgba
}

pub(super) fn tile_of(block: &RenderBlock, face: usize) -> Tile {
    Tile { layer: block.textures[face], tint: block.tint[face], material: block.material[face] }
}

/// A box's six faces in [`crate::assets::FACE_NAMES`] order, with UVs that follow the block's
/// texel grid as the terrain's do.
fn box_faces(b: Box16, tile: impl Fn(usize) -> Tile) -> impl Iterator<Item = Face> {
    let min = Vec3::new(b[0].into(), b[1].into(), b[2].into()) / 16.0;
    let max = Vec3::new(b[3].into(), b[4].into(), b[5].into()) / 16.0;
    (0..6).filter_map(move |face| {
        // Corners counter-clockwise from outside, then each corner's texel position.
        let (corners, uv): ([Vec3; 4], fn(Vec3) -> [f32; 2]) = match face {
            0 => (quad_corners(max.x, [min.z, max.z], [min.y, max.y], |x, z, y| Vec3::new(x, y, z), true), |p| [1.0 - p.z, 1.0 - p.y]),
            1 => (quad_corners(min.x, [min.z, max.z], [min.y, max.y], |x, z, y| Vec3::new(x, y, z), false), |p| [p.z, 1.0 - p.y]),
            2 => (quad_corners(max.y, [min.x, max.x], [min.z, max.z], |y, x, z| Vec3::new(x, y, z), true), |p| [p.x, p.z]),
            3 => (quad_corners(min.y, [min.x, max.x], [min.z, max.z], |y, x, z| Vec3::new(x, y, z), false), |p| [p.x, 1.0 - p.z]),
            4 => (quad_corners(max.z, [min.x, max.x], [min.y, max.y], |z, x, y| Vec3::new(x, y, z), false), |p| [p.x, 1.0 - p.y]),
            _ => (quad_corners(min.z, [min.x, max.x], [min.y, max.y], |z, x, y| Vec3::new(x, y, z), true), |p| [1.0 - p.x, 1.0 - p.y]),
        };
        let flat = (max - min).cmpeq(Vec3::ZERO).any();
        (!flat).then(|| Face { corners, uv: corners.map(uv), tile: tile(face) })
    })
}

/// The four corners of a face on plane `at`, spanning `a` and `b`; `flip` reverses the winding.
fn quad_corners(at: f32, a: [f32; 2], b: [f32; 2], place: impl Fn(f32, f32, f32) -> Vec3, flip: bool) -> [Vec3; 4] {
    let c = [place(at, a[0], b[0]), place(at, a[1], b[0]), place(at, a[1], b[1]), place(at, a[0], b[1])];
    if flip { [c[0], c[3], c[2], c[1]] } else { c }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> Atlas {
        Atlas { layers: vec![Texture { rgba: Box::new([200; TEXEL_BYTES]) }], animations: Vec::new() }
    }

    fn cube() -> RenderBlock {
        let mut block = crate::blocks::BlockTable::cube(0);
        block.material = [Material::Opaque; 6];
        block
    }

    #[test]
    fn a_cube_is_six_outward_faces_over_one_tile() {
        let skin = skin(&cube(), &atlas()).unwrap();
        let mesh = skin.mesh.unwrap();
        assert_eq!((mesh.vertices.len(), skin.height), (36, 16));
        for v in &mesh.vertices {
            let p = Vec3::from(v.position);
            assert!(p.dot(Vec3::from(v.normal)) > 0.49, "{v:?}");
        }
    }

    #[test]
    fn tints_multiply_and_opaque_faces_lose_alpha() {
        let tile = Tile { layer: 0, tint: Tint::Fixed([255, 0, 128]), material: Material::Opaque };
        let rgba = texels(Some(&Texture { rgba: Box::new([200; TEXEL_BYTES]) }), tile);
        assert_eq!(&rgba[..4], &[200, 0, 100, 255]);
    }
}
