//! A model drawn as the entity pass draws it: the look's mesh, `Mesh::skin`'s bone matrices, the
//! pass's placement (`gpu/entities.rs`) and the shader's normal (`entity.wgsl`: the bone matrix
//! times the normal, normalised).

use std::collections::BTreeMap;
use std::sync::Arc;

use acacia_render::entity::Pose;
use acacia_render::entity::java::{Part, Poses, mesh};
use glam::{Mat4, Vec3, Vec4};
use serde_json::Value;

use crate::compare::{Case, Report, floats};

/// Per drawn part path, its corners `[x, y, z, nx, ny, nz, u, v]` in the world around the entity.
pub fn corners(model: &Part, poses: &Poses, yaw: f32, scale: f32) -> BTreeMap<String, Vec<[f32; 8]>> {
    let shared = Arc::new(model.clone());
    let skin = mesh::bake(&shared).skin(&Pose::java(poses.clone()));
    let body = Mat4::from_rotation_y(-yaw.to_radians()) * Mat4::from_scale(Vec3::new(scale, scale, -scale));
    let mut out = BTreeMap::new();
    for (index, (path, part)) in model.walk().into_iter().enumerate() {
        let bone = skin[index.min(skin.len() - 1)];
        if part.polygons.is_empty() || bone == Mat4::ZERO {
            continue;
        }
        let m = body * bone;
        let corner = |&[x, y, z, u, v]: &[f32; 5], n: [f32; 3]| {
            let p = m.transform_point3(Vec3::new(x, y, z) / 16.0);
            let n = (m * Vec4::new(n[0], n[1], n[2], 0.0)).truncate().normalize_or_zero();
            [p.x, p.y, p.z, n.x, n.y, n.z, u, v]
        };
        out.insert(path, part.polygons.iter().flat_map(|q| q.vertices.iter().map(move |c| corner(c, q.normal))).collect());
    }
    out
}

/// Each part's pose `[x, y, z, xRot, yRot, zRot, xScale, yScale, zScale, visible, skipDraw]`.
pub fn part_poses(model: &Part, poses: &Poses) -> BTreeMap<String, Vec<f32>> {
    let flag = |b: bool| f32::from(u8::from(b));
    model.walk().into_iter().enumerate().map(|(i, (path, _))| {
        let (pose, visible, skip) = poses.at(i);
        let mut row = pose.to_array().to_vec();
        row.extend([flag(visible), flag(skip)]);
        (path, row)
    }).collect()
}

fn compare_parts(case: &mut Case, expected: &Value, got: &BTreeMap<String, Vec<f32>>) {
    let expected = expected.as_object().cloned().unwrap_or_default();
    for (path, row) in &expected {
        match got.get(path) {
            Some(ours) => case.numbers(&format!("part {path}"), &floats(row), ours, 1e-5),
            None => case.diverge(&format!("part {path}"), "present", "absent"),
        }
    }
    for path in got.keys().filter(|p| !expected.contains_key(*p)) {
        case.diverge(&format!("part {path}"), "absent", "present");
    }
}

/// `layers.jsonl` of entity_models: the rest pose at yaw 0.
pub fn rest(layers: &BTreeMap<String, Part>, record: &Value, report: &mut Report) {
    let name = record["case"].as_str().unwrap_or_default();
    let mut case = report.case(name);
    let Some(model) = layers.get(name) else { return case.diverge("layer", "present", "absent in the look") };
    compare_posed(&mut case, record, model, &Poses::new(model));
}

/// A record's `parts` and `drawn` against `model` drawn with `poses` at yaw 0.
pub fn compare_posed(case: &mut Case, record: &Value, model: &Part, poses: &Poses) {
    compare_parts(case, &record["parts"], &part_poses(model, poses));
    let ours = corners(model, poses, 0.0, 1.0);
    let expected = record["drawn"].as_object().cloned().unwrap_or_default();
    for (path, list) in &expected {
        let Some(got) = ours.get(path) else {
            case.diverge(&format!("drawn {path}"), "drawn", "not drawn");
            continue;
        };
        let want: Vec<Vec<f32>> = list.as_array().into_iter().flatten().map(floats).collect();
        if want.len() != got.len() {
            case.diverge(&format!("drawn {path}.corners"), want.len(), got.len());
            continue;
        }
        for (i, (w, g)) in want.iter().zip(got).enumerate() {
            let (wp, gp) = (&w[..3], &g[..3]);
            case.numbers(&format!("drawn {path}[{i}].position"), wp, gp, 1e-5);
            case.numbers(&format!("drawn {path}[{i}].normal"), &w[3..6], &g[3..6], 1e-4);
            case.numbers(&format!("drawn {path}[{i}].uv"), &w[6..8], &g[6..8], 1e-6);
        }
    }
    for path in ours.keys().filter(|p| !expected.contains_key(*p)) {
        case.diverge(&format!("drawn {path}"), "not drawn", "drawn");
    }
}
