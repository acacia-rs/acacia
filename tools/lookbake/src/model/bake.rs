// Ported from Pomme (https://github.com/PommeMC/Client), pomme-client/src/world/block/model.rs.
// Copyright (C) 2026 Purdze. GPL-3.0-or-later; see ../../LICENSE-pomme.

//! Model elements to faces, after vanilla's `FaceBakery`. Positions are in 1/16 block, texture
//! coordinates in texels.

use glam::Vec3;

use super::{Resolved, turn};
use crate::blockstate::ModelRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

use Direction::{Down, East, North, South, Up, West};

const DIRECTIONS: [Direction; 6] = [Down, Up, North, South, West, East];

impl Direction {
    pub fn named(name: &str) -> Option<Direction> {
        DIRECTIONS.into_iter().find(|d| format!("{d:?}").eq_ignore_ascii_case(name))
    }

    fn normal(self) -> Vec3 {
        match self {
            Down => Vec3::NEG_Y,
            Up => Vec3::Y,
            North => Vec3::NEG_Z,
            South => Vec3::Z,
            West => Vec3::NEG_X,
            East => Vec3::X,
        }
    }

    /// Index in the renderer's face order: east, west, up, down, south, north.
    pub fn face(self) -> u8 {
        match self {
            East => 0,
            West => 1,
            Up => 2,
            Down => 3,
            South => 4,
            North => 5,
        }
    }

    fn turned(self, degrees: u16, cycle: [Direction; 4]) -> Direction {
        let Some(at) = cycle.iter().position(|d| *d == self) else { return self };
        cycle[(at + usize::from(degrees / 90)) % 4]
    }

    /// After a blockstate's `x` then `y` rotation.
    fn rotated(self, x: u16, y: u16) -> Direction {
        self.turned(x, [North, Down, South, Up]).turned(y, [North, East, South, West])
    }

    /// Where a point on a face of this direction lands in the texture when the texture is laid
    /// on the block unrotated: vanilla's default face UVs.
    pub fn project(self, [x, y, z]: [f32; 3]) -> [f32; 2] {
        match self {
            Down => [x, 16.0 - z],
            Up => [x, z],
            North => [16.0 - x, 16.0 - y],
            South => [x, 16.0 - y],
            West => [z, 16.0 - y],
            East => [16.0 - z, 16.0 - y],
        }
    }

    /// The point of the block's mid-plane that [`Direction::project`] takes to `[u, v]`.
    fn unproject(self, [u, v]: [f32; 2]) -> [f32; 3] {
        match self {
            Down => [u, 8.0, 16.0 - v],
            Up => [u, 8.0, v],
            North => [16.0 - u, 16.0 - v, 8.0],
            South => [u, 16.0 - v, 8.0],
            West => [8.0, 16.0 - v, u],
            East => [8.0, 16.0 - v, 16.0 - u],
        }
    }
}

/// UnbakedCuboidGeometry.bake: a flat element draws only the two faces across its thin axis.
fn drawn(direction: Direction, from: [f32; 3], to: [f32; 3]) -> bool {
    let axis = usize::from(direction.face() / 2);
    (0..3).all(|other| other == axis || from[other] != to[other])
}

#[derive(Debug, Clone, PartialEq)]
pub struct BakedFace {
    /// Counter-clockwise seen from outside.
    pub positions: [[f32; 3]; 4],
    pub uvs: [[f32; 2]; 4],
    /// Texture id: `block/oak_planks`.
    pub texture: String,
    pub cull: Option<Direction>,
    /// Which of the block's tint colours it takes.
    pub tint_index: Option<i32>,
    /// The direction it is shaded as; `None` for full brightness.
    pub shade: Option<Direction>,
    pub ambient_occlusion: bool,
}

/// The faces of `model` as a blockstate places it.
pub fn bake(model: &Resolved, placed: &ModelRef) -> Vec<BakedFace> {
    let mut faces = Vec::new();
    for element in &model.elements {
        for (name, face) in &element.faces {
            let (Some(direction), Some(texture)) = (Direction::named(name), model.texture(&face.texture)) else { continue };
            if !drawn(direction, element.from, element.to) {
                continue;
            }
            let mut positions = turn::element(face_positions(direction, element.from, element.to), element.rotation.as_ref());
            let mut uvs = face_uvs(direction, element.from, element.to, face.uv, face.rotation);
            let mut cull = face.cullface.as_deref().and_then(Direction::named);
            if placed.x != 0 || placed.y != 0 {
                positions = positions.map(|p| turn::model(p, placed.x, placed.y));
                cull = cull.map(|d| d.rotated(placed.x, placed.y));
            }
            // FaceBakery.bakeQuad: the shade direction is the turned quad's nearest cardinal, up when degenerate.
            let facing = nearest_direction(&positions).unwrap_or(Up);
            if placed.uvlock && (placed.x != 0 || placed.y != 0) {
                // Not in Pomme. The texture stays aligned to the world: each corner's place on the
                // unturned block's side goes where the model's turn takes it.
                let turned = direction.rotated(placed.x, placed.y);
                uvs = uvs.map(|uv| turned.project(turn::model(direction.unproject(uv), placed.x, placed.y)));
            }
            // Shaded as up is full brightness, which is what `shade: false` meant before 26.1.
            let shade = match element.shade_direction_override.as_deref().and_then(Direction::named) {
                _ if !element.shade => None,
                Some(Up) => None,
                Some(direction) => Some(direction),
                None => Some(facing),
            };
            let tint_index = face.tintindex.filter(|index| *index >= 0);
            faces.push(BakedFace { positions, uvs, texture: texture.to_owned(), cull, tint_index, shade, ambient_occlusion: model.ambient_occlusion });
        }
    }
    faces
}

/// Vanilla `FaceInfo`: the vertex order the UV rules are defined against.
fn face_positions(direction: Direction, [x0, y0, z0]: [f32; 3], [x1, y1, z1]: [f32; 3]) -> [[f32; 3]; 4] {
    match direction {
        Down => [[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]],
        Up => [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        North => [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]],
        South => [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]],
        West => [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
        East => [[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]],
    }
}

fn face_uvs(direction: Direction, from: [f32; 3], to: [f32; 3], explicit: Option<[f32; 4]>, rotation: Option<i32>) -> [[f32; 2]; 4] {
    // FaceBakery.defaultFaceUV: the window the face covers when the texture lies on the block.
    let [u1, v1, u2, v2] = explicit.unwrap_or_else(|| {
        let ([a, b], [c, d]) = (direction.project(face_positions(direction, from, to)[0]), direction.project(face_positions(direction, from, to)[2]));
        [a, b, c, d]
    });
    // CuboidFace.UVs: one corner cycle for every face; a rotation shifts each vertex along it,
    // turning the texture clockwise seen from outside.
    let cycle = [[u1, v1], [u1, v2], [u2, v2], [u2, v1]];
    let shift = rotation.map_or(0, |r| r.rem_euclid(360) / 90) as usize;
    std::array::from_fn(|i| cycle[(i + shift) % 4])
}

/// FaceBakery.findClosestDirection: the cardinal the winding normal points along most, first in
/// [`DIRECTIONS`] order on a tie; `None` for a degenerate quad.
fn nearest_direction(positions: &[[f32; 3]; 4]) -> Option<Direction> {
    let [p0, p1, p2] = [0, 1, 2].map(|i| Vec3::from_array(positions[i]));
    let normal = (p1 - p0).cross(p2 - p0).try_normalize()?;
    let mut best = (None, 0.0f32);
    for direction in DIRECTIONS {
        let along = normal.dot(direction.normal());
        if along > best.1 {
            best = (Some(direction), along);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Element, ElementRotation, FaceDef};

    fn cube(faces: &[(&str, Option<[f32; 4]>, Option<&str>)]) -> Resolved {
        let face = |(name, uv, cull): &(&str, Option<[f32; 4]>, Option<&str>)| {
            (name.to_string(), FaceDef { uv: *uv, texture: "#all".into(), cullface: cull.map(str::to_owned), rotation: None, tintindex: None })
        };
        let element = Element { from: [0.0; 3], to: [16.0; 3], rotation: None, faces: faces.iter().map(face).collect(), shade: true, shade_direction_override: None };
        Resolved { textures: [("all".to_owned(), "block/stone".to_owned())].into(), elements: vec![element], ambient_occlusion: true }
    }

    fn placed(x: u16, y: u16, uvlock: bool) -> ModelRef {
        ModelRef { model: "block/stone".into(), x, y, uvlock }
    }

    fn rounded<const N: usize>(points: [[f32; N]; 4]) -> [[i32; N]; 4] {
        points.map(|p| p.map(|v| v.round() as i32))
    }

    #[test]
    fn faces_wind_counter_clockwise_and_take_the_default_window() {
        let faces = bake(&cube(&[("up", None, Some("up")), ("east", None, None), ("down", None, None)]), &placed(0, 0, false));
        for face in &faces {
            let direction = nearest_direction(&face.positions).unwrap();
            assert_eq!(face.shade, Some(direction));
            assert_eq!(face.uvs, face.positions.map(|p| direction.project(p)), "{direction:?}");
        }
        let up = faces.iter().find(|f| f.shade == Some(Up)).unwrap();
        assert_eq!((up.cull, up.texture.as_str()), (Some(Up), "block/stone"));
        assert_eq!(rounded(up.uvs), [[0, 0], [0, 16], [16, 16], [16, 0]]);
    }

    #[test]
    fn a_blockstate_rotation_turns_faces_and_uvlock_keeps_the_texture_still() {
        let model = cube(&[("north", Some([0.0, 0.0, 8.0, 16.0]), Some("north"))]);
        // y = 90 turns north to east.
        let [turned] = bake(&model, &placed(0, 90, false)).try_into().unwrap();
        assert_eq!((turned.cull, turned.shade), (Some(East), Some(East)));
        assert!(turned.positions.iter().all(|p| (p[0] - 16.0).abs() < 1e-4));
        assert_eq!(rounded(turned.uvs), [[0, 0], [0, 16], [8, 16], [8, 0]], "the window turns with the face");
        let [locked] = bake(&model, &placed(0, 90, true)).try_into().unwrap();
        assert_eq!(rounded(locked.uvs), rounded(turned.uvs), "a turn about y leaves a side's window alone");
        let [top] = bake(&cube(&[("up", None, None)]), &placed(0, 90, true)).try_into().unwrap();
        assert_eq!(rounded(top.uvs), rounded(top.positions.map(|p| Up.project(p))), "and turns the top's back");
        // x = 90 turns up to north.
        let [tipped] = bake(&cube(&[("up", None, Some("up"))]), &placed(90, 0, false)).try_into().unwrap();
        assert_eq!((tipped.cull, tipped.shade), (Some(North), Some(North)));
    }

    #[test]
    fn an_element_rotation_with_rescale_reaches_the_block_edge() {
        let rotation = ElementRotation { origin: [8.0, 8.0, 8.0], axis: Some("y".into()), angle: 45.0, x: 0.0, y: 0.0, z: 0.0, rescale: true };
        let face = FaceDef { uv: None, texture: "#all".into(), cullface: None, rotation: None, tintindex: Some(0) };
        let faces = [("south".to_owned(), face)].into();
        let plane = Element { from: [0.0, 0.0, 8.0], to: [16.0, 16.0, 8.0], rotation: Some(rotation), faces, shade: true, shade_direction_override: Some("up".into()) };
        let model = Resolved { textures: [("all".to_owned(), "block/poppy".to_owned())].into(), elements: vec![plane], ambient_occlusion: false };
        let [face] = bake(&model, &placed(0, 0, false)).try_into().unwrap();
        assert!(face.tint_index == Some(0) && face.shade.is_none());
        let xs: Vec<i32> = face.positions.iter().map(|p| p[0].round() as i32).collect();
        assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&0), Some(&16)), "the diagonal spans the block");
    }
}
