//! `ModelPart.render`: the walk down a posed model on a pose stack, each part's
//! `translateAndRotate` then its cubes' quads (`Cube.compile`).

use glam::{Mat3, Mat4, Vec3};

use super::model::Part;
use super::poses::Poses;

/// One corner as `Cube.compile` hands it to the vertex consumer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// A part that draws its own quads, with its path in the tree (`body/head`).
#[derive(Debug, Clone)]
pub struct DrawnPart {
    pub path: String,
    /// Index in [`Part::walk`] order.
    pub index: usize,
    pub matrix: Mat4,
    /// Four per polygon.
    pub vertices: Vec<DrawnVertex>,
}

/// Per part in [`Part::walk`] order: its pose-stack matrix under `base`, or `None` when it does
/// not draw (it or an ancestor is hidden, or it is scaled to nothing).
pub fn part_matrices(model: &Part, poses: &Poses, base: Mat4) -> Vec<Option<Mat4>> {
    let mut out = Vec::new();
    visit(model, poses, Some(base), &mut out);
    out
}

/// Every part's `translateAndRotate` chain under `base`, walk order, shown or not: what layers
/// that place things on a part (`translateToHand`, `translateToHead`) use.
pub fn part_frames(model: &Part, poses: &Poses, base: Mat4) -> Vec<Mat4> {
    fn walk(part: &Part, poses: &Poses, parent: Mat4, out: &mut Vec<Mat4>) {
        let matrix = parent * poses.at(out.len()).0.matrix();
        out.push(matrix);
        for child in &part.children {
            walk(child, poses, matrix, out);
        }
    }
    let mut out = Vec::new();
    walk(model, poses, base, &mut out);
    out
}

fn visit(part: &Part, poses: &Poses, parent: Option<Mat4>, out: &mut Vec<Option<Mat4>>) {
    let (pose, visible, _) = poses.at(out.len());
    let collapsed = pose.x_scale == 0.0 && pose.y_scale == 0.0 && pose.z_scale == 0.0;
    // Java's ModelPart.render returns for an empty part too; nothing below it can draw then.
    let empty = part.polygons.is_empty() && part.children.is_empty();
    let matrix = parent.filter(|_| visible && !collapsed && !empty).map(|m| m * pose.matrix());
    out.push(matrix);
    for child in &part.children {
        visit(child, poses, matrix, out);
    }
}

/// The posed model's quads under `base`, part by part.
pub fn draw(model: &Part, poses: &Poses, base: Mat4) -> Vec<DrawnPart> {
    let matrices = part_matrices(model, poses, base);
    let mut out = Vec::new();
    for (index, ((path, part), matrix)) in model.walk().into_iter().zip(matrices).enumerate() {
        let Some(matrix) = matrix.filter(|_| !poses.at(index).2 && !part.polygons.is_empty()) else { continue };
        let normals = Mat3::from_mat4(matrix).inverse().transpose();
        let mut vertices = Vec::with_capacity(part.polygons.len() * 4);
        for polygon in &part.polygons {
            let normal = (normals * Vec3::from(polygon.normal)).normalize_or_zero().to_array();
            vertices.extend(polygon.vertices.iter().map(|&[x, y, z, u, v]| DrawnVertex {
                position: matrix.transform_point3(Vec3::new(x, y, z) / 16.0).to_array(),
                normal,
                uv: [u, v],
            }));
        }
        out.push(DrawnPart { path, index, matrix, vertices });
    }
    out
}

/// `LivingEntityRenderer`'s root transform without the entity's position: turned by
/// `180 - yaw` about y, mirrored in x and y, the renderer's `scale`, and down by 1.501.
pub fn living_root(yaw_degrees: f32, scale: Vec3) -> Mat4 {
    Mat4::from_rotation_y((180.0 - yaw_degrees).to_radians())
        * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        * Mat4::from_scale(scale)
        * Mat4::from_translation(Vec3::new(0.0, -1.501, 0.0))
}

#[cfg(test)]
mod tests {
    use super::super::model::{parse_layers, tests::SAMPLE};
    use super::*;

    #[test]
    fn a_part_moves_by_its_parents_pose_then_its_own() {
        let model = parse_layers(SAMPLE).unwrap().remove("minecraft:test#main").unwrap();
        let mut poses = Poses::new(&model);
        let drawn = draw(&model, &poses, Mat4::IDENTITY);
        assert_eq!(drawn.len(), 1);
        // The body's first corner, (-4, 0, -2) pixels, 12 pixels down.
        assert_eq!(drawn[0].vertices[0].position, [-0.25, 0.75, -0.125]);
        // The head has no cubes and no children: ModelPart.render returns before moving it.
        let matrices = part_matrices(&model, &poses, Mat4::IDENTITY);
        assert_eq!((matrices[1].map(|m| m.transform_point3(Vec3::ZERO)), matrices[2]), (Some(Vec3::new(0.0, 0.75, 0.0)), None));
        poses.get("body").y_rot = std::f32::consts::PI;
        let turned = draw(&model, &poses, Mat4::IDENTITY);
        assert!(Vec3::from(turned[0].vertices[0].normal).abs_diff_eq(Vec3::Z, 1e-6));
        poses.set_visible("body", false);
        assert!(draw(&model, &poses, Mat4::IDENTITY).is_empty());
        assert_eq!(part_matrices(&model, &poses, Mat4::IDENTITY)[2], None);
    }
}
