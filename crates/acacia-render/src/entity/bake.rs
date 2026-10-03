//! Turns a [`Geometry`] into triangles in its rest pose. Conventions: see README "Entities".

use glam::{Mat4, Vec3};

use super::geometry::{Bone, Corner, Cube, Geometry, Uv};

/// Vertices whose bone is the head or below it turn with the head.
pub const PART_HEAD: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Raw model space, in blocks.
    pub position: [f32; 3],
    pub part: u32,
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Triangle list.
    pub vertices: Vec<Vertex>,
    /// Point the head turns around, in blocks.
    pub head_pivot: Vec3,
}

/// Degrees as written in geometry files to a matrix; the file's x and z angles run clockwise.
fn rotation(deg: [f32; 3]) -> Mat4 {
    let [x, y, z] = deg.map(f32::to_radians);
    Mat4::from_rotation_z(-z) * Mat4::from_rotation_y(y) * Mat4::from_rotation_x(-x)
}

fn around(pivot: [f32; 3], deg: [f32; 3]) -> Mat4 {
    let p = Vec3::from(pivot);
    Mat4::from_translation(p) * rotation(deg) * Mat4::from_translation(-p)
}

pub fn bake(geometry: &Geometry) -> Mesh {
    let mut mesh = Mesh::default();
    for bone in &geometry.bones {
        let (parents, head) = chain(geometry, bone);
        let matrix = parents * around(bone.pivot, bone.rotation) * around(bone.pivot, bone.bind_pose_rotation);
        if bone.name.eq_ignore_ascii_case("head") {
            mesh.head_pivot = parents.transform_point3(Vec3::from(bone.pivot)) / 16.0;
        }
        for cube in &bone.cubes {
            let m = cube.rotation.map_or(matrix, |(deg, pivot)| matrix * around(pivot, deg));
            push_cube(&mut mesh.vertices, cube, m, geometry.texture_size, u32::from(head));
        }
        for poly in &bone.polys {
            let vertex = |&(position, normal, uv): &Corner| Vertex {
                position: (matrix.transform_point3(Vec3::from(position)) / 16.0).to_array(),
                part: u32::from(head),
                normal: matrix.transform_vector3(Vec3::from(normal)).normalize_or_zero().to_array(),
                uv,
            };
            // A fan from the first corner: polygons are convex quads or triangles.
            for i in 1..poly.len().saturating_sub(1) {
                mesh.vertices.extend([&poly[0], &poly[i], &poly[i + 1]].map(vertex));
            }
        }
    }
    mesh
}

/// The product of the ancestors' transforms, and whether the bone is the head or under it.
fn chain(geometry: &Geometry, bone: &Bone) -> (Mat4, bool) {
    let (mut matrix, mut head) = (Mat4::IDENTITY, bone.name.eq_ignore_ascii_case("head"));
    let mut parent = bone.parent.as_deref();
    // Bounded: a malformed file could name a bone its own ancestor.
    for _ in 0..32 {
        let Some(p) = parent.and_then(|name| geometry.bones.iter().find(|b| b.name == name)) else { break };
        matrix = around(p.pivot, p.rotation) * matrix;
        head |= p.name.eq_ignore_ascii_case("head");
        parent = p.parent.as_deref();
    }
    (matrix, head)
}

/// Faces in [`super::geometry::FACES`] order: normal, then corners as (x, y, z) picks of min (0)
/// or max (1), ordered like the texture rectangle: top-left, top-right, bottom-right, bottom-left.
const FACE_CORNERS: [([f32; 3], [[usize; 3]; 4]); 6] = [
    ([1.0, 0.0, 0.0], [[1, 1, 0], [1, 1, 1], [1, 0, 1], [1, 0, 0]]),
    ([-1.0, 0.0, 0.0], [[0, 1, 1], [0, 1, 0], [0, 0, 0], [0, 0, 1]]),
    ([0.0, 1.0, 0.0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]]),
    ([0.0, -1.0, 0.0], [[0, 0, 1], [1, 0, 1], [1, 0, 0], [0, 0, 0]]),
    ([0.0, 0.0, 1.0], [[1, 1, 1], [0, 1, 1], [0, 0, 1], [1, 0, 1]]),
    ([0.0, 0.0, -1.0], [[0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 0, 0]]),
];

/// `(u, v, width, height)` of each face in the unfolded box at `uv`.
fn box_rects([u, v]: [f32; 2], [w, h, d]: [f32; 3], mirror: bool) -> [[f32; 4]; 6] {
    let (near, far) = ([u, v + d, d, h], [u + d + w, v + d, d, h]);
    let (plus_x, minus_x) = if mirror { (near, far) } else { (far, near) };
    let rects = [plus_x, minus_x, [u + d, v, w, d], [u + d + w, v, w, d], [u + d + w + d, v + d, w, h], [u + d, v + d, w, h]];
    if mirror { rects.map(|[u, v, w, h]| [u + w, v, -w, h]) } else { rects }
}

fn push_cube(out: &mut Vec<Vertex>, cube: &Cube, matrix: Mat4, texture: [f32; 2], part: u32) {
    let min = Vec3::from(cube.origin) - cube.inflate;
    let max = Vec3::from(cube.origin) + Vec3::from(cube.size) + cube.inflate;
    let rects = match &cube.uv {
        Uv::Box(uv) => box_rects(*uv, cube.size, cube.mirror).map(Some),
        Uv::Faces(faces) => faces.map(|f| f.map(|([u, v], [w, h])| [u, v, w, h])),
    };
    for ((normal, corners), rect) in FACE_CORNERS.iter().zip(rects) {
        let Some([u, v, w, h]) = rect else { continue };
        let normal = matrix.transform_vector3(Vec3::from(*normal)).normalize_or_zero().to_array();
        let uvs = [[u, v], [u + w, v], [u + w, v + h], [u, v + h]];
        let vertex = |i: usize| {
            let pick = corners[i];
            let p = Vec3::new(
                if pick[0] == 0 { min.x } else { max.x },
                if pick[1] == 0 { min.y } else { max.y },
                if pick[2] == 0 { min.z } else { max.z },
            );
            let position = (matrix.transform_point3(p) / 16.0).to_array();
            Vertex { position, part, normal, uv: [uvs[i][0] / texture[0], uvs[i][1] / texture[1]] }
        };
        out.extend([0, 1, 2, 0, 2, 3].map(vertex));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(origin: [f32; 3], size: [f32; 3]) -> Cube {
        Cube { origin, size, inflate: 0.0, mirror: false, rotation: None, uv: Uv::Box([0.0, 0.0]) }
    }

    fn bounds(mesh: &Mesh) -> (Vec3, Vec3) {
        let points = mesh.vertices.iter().map(|v| Vec3::from(v.position));
        points.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
    }

    #[test]
    fn cube_rotation_lays_the_cow_body_flat() {
        // cow.v2's body: stored upright, pitched 90° into place.
        let body = Cube { rotation: Some(([90.0, 0.0, 0.0], [0.0, 18.0, 20.0])), ..cube([-6.0, 29.0, 14.0], [12.0, 18.0, 10.0]) };
        let bone = Bone { name: "body".into(), cubes: vec![body], ..Default::default() };
        let (lo, hi) = bounds(&bake(&Geometry { texture_size: [64.0, 64.0], bones: vec![bone] }));
        assert!((lo * 16.0 - Vec3::new(-6.0, 12.0, -9.0)).abs().max_element() < 1e-3, "{lo}");
        assert!((hi * 16.0 - Vec3::new(6.0, 22.0, 9.0)).abs().max_element() < 1e-3, "{hi}");
    }

    #[test]
    fn box_uv_puts_the_front_right_of_the_first_side() {
        let head = Bone { name: "head".into(), pivot: [0.0, 24.0, 0.0], cubes: vec![cube([-4.0, 24.0, -4.0], [8.0; 3])], ..Default::default() };
        let hat = Bone { name: "hat".into(), parent: Some("head".into()), ..Default::default() };
        let mesh = bake(&Geometry { texture_size: [64.0, 64.0], bones: vec![head, hat] });
        assert_eq!(mesh.head_pivot, Vec3::new(0.0, 1.5, 0.0));
        assert!(mesh.vertices.iter().all(|v| v.part == PART_HEAD));
        // The front (-z) face is the sixth: its top-left texel is (8, 8) of 64 and sits at -x.
        let front = &mesh.vertices[30..36];
        assert_eq!((front[0].uv, front[0].position), ([0.125, 0.125], [-0.25, 2.0, -0.25]));
        assert_eq!(front[2].uv, [0.25, 0.25]);
        assert_eq!(front[0].normal, [0.0, 0.0, -1.0]);
    }
}
