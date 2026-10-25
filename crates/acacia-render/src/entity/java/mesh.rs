//! A Java model as the entity pass draws it: one bone per part, its quads in the part's own space,
//! and per frame each part's `ModelPart.render` matrix carried into acacia's model space.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::draw::part_matrices;
use super::model::Part;
use super::poses::Poses;
use crate::entity::bake::{Joint, MAX_BONES, Mesh, Vertex};

/// Java's model space into acacia's (README "Model space"): `LivingEntityRenderer`'s
/// `translate(0, -1.501, 0)`, and its `scale(-1, -1, 1)` and `180 - yaw` turn against acacia's
/// mirrored z, which leave y flipped.
pub fn to_model_space() -> Mat4 {
    Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::new(0.0, -1.501, 0.0))
}

/// The model's quads as two triangles each, every vertex on its part's bone ([`Part::walk`]
/// order). Parts past [`MAX_BONES`] share the last bone, as Bedrock meshes do.
pub fn bake(model: &Arc<Part>) -> Mesh {
    let parts = model.walk();
    let mut mesh = Mesh { bones: parts.iter().map(|(_, p)| p.name.clone()).collect(), java: Some(model.clone()), ..Mesh::default() };
    let mut parents = vec![None; parts.len()];
    fn link(part: &Part, index: &mut usize, parent: Option<usize>, parents: &mut [Option<usize>]) {
        let own = *index;
        parents[own] = parent;
        for child in &part.children {
            *index += 1;
            link(child, index, Some(own), parents);
        }
    }
    link(model, &mut 0, None, &mut parents);
    for (index, (_, part)) in parts.iter().enumerate() {
        let bone = index.min(MAX_BONES - 1) as u32;
        mesh.joints.push(Joint { parent: parents[index], pivot: Vec3::ZERO, rotation: [0.0; 3], unbind: Mat4::IDENTITY });
        for polygon in &part.polygons {
            let corner = |i: usize| {
                let [x, y, z, u, v] = polygon.vertices[i];
                Vertex { position: (Vec3::new(x, y, z) / 16.0).to_array(), bone, normal: polygon.normal, uv: [u, v] }
            };
            mesh.vertices.extend([0, 1, 2, 0, 2, 3].map(corner));
        }
    }
    mesh
}

/// Per bone, the posed part's matrix in acacia's model space; a part that does not draw
/// (hidden, scaled to nothing, `skipDraw`) collapses to a point.
pub fn skin(model: &Part, poses: &Poses) -> Vec<Mat4> {
    let matrices = part_matrices(model, poses, to_model_space());
    let posed = |(index, m): (usize, Option<Mat4>)| m.filter(|_| !poses.at(index).2).unwrap_or(Mat4::ZERO);
    matrices.into_iter().enumerate().take(MAX_BONES).map(posed).collect()
}

#[cfg(test)]
mod tests {
    use super::super::model::{parse_layers, tests::SAMPLE};
    use super::*;

    #[test]
    fn the_rest_pose_stands_on_the_ground() {
        let model = Arc::new(parse_layers(SAMPLE).unwrap().remove("minecraft:test#main").unwrap());
        let mesh = bake(&model);
        assert_eq!((mesh.bones.len(), mesh.vertices.len(), mesh.joints[2].parent), (3, 6, Some(1)));
        let matrices = skin(&model, &Poses::new(&model));
        // The body's lowest corner, 24 pixels down in Java's space, lands 0.001 above the feet.
        let foot = matrices[1].transform_point3(Vec3::from(mesh.vertices[2].position));
        assert!(foot.abs_diff_eq(Vec3::new(0.25, 0.001, -0.125), 1e-5), "{foot}");
        assert_eq!(matrices[2], Mat4::ZERO);
    }
}
