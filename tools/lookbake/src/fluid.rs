//! The Java look's [`FluidRules`]: Java's over the Bedrock-derived ones. `#blocks_fluid_flow` and
//! legacy solidity come from the jar's block tags (in 26.3 `isSolid` is exactly that tag less the
//! cobweb and bamboo sapling); the faces a shape covers from the Java model where Java draws the
//! block. tools/parity's `fluids` harness checks the result against the game for every state.

use std::collections::HashSet;
use std::path::Path;

use acacia_render::blocks::FluidRules;
use acacia_render::blocks::fluid::{no_occlusion, plane_coverage};
use serde_json::Value;

use crate::download::Error;
use crate::mapping::JavaState;
use crate::model::bake::BakedFace;

/// Solid though not in `#blocks_fluid_flow`.
const SOLID_UNTAGGED: [&str; 2] = ["cobweb", "bamboo_sapling"];
/// Drawn as full cubes, but with no face sturdy (as leaves, `#leaves`): their support shape is
/// their smaller or empty collision shape.
const NO_SUPPORT: [&str; 2] = ["honey_block", "powder_snow"];

/// The block tags the rules read, as sets of block names without namespace.
pub struct Tags {
    flow: HashSet<String>,
    leaves: HashSet<String>,
}

impl Tags {
    /// `dir` is the jar's `data/minecraft/tags/block`.
    pub fn load(dir: &Path) -> Result<Tags, Error> {
        Ok(Tags { flow: resolve(dir, "blocks_fluid_flow")?, leaves: resolve(dir, "leaves")? })
    }
}

/// A tag's blocks, through the tags it names.
fn resolve(dir: &Path, tag: &str) -> Result<HashSet<String>, Error> {
    let mut out = HashSet::new();
    let (mut pending, mut seen) = (vec![tag.to_owned()], HashSet::new());
    while let Some(tag) = pending.pop() {
        if !seen.insert(tag.clone()) {
            continue;
        }
        let file: Value = serde_json::from_slice(&std::fs::read(dir.join(format!("{tag}.json")))?)?;
        for entry in file["values"].as_array().ok_or_else(|| format!("tag {tag} has no values"))? {
            let id = entry.as_str().or_else(|| entry["id"].as_str()).ok_or_else(|| format!("tag {tag}: bad entry {entry}"))?;
            match id.strip_prefix('#') {
                Some(nested) => pending.push(unspaced(nested)),
                None => {
                    out.insert(unspaced(id));
                }
            }
        }
    }
    Ok(out)
}

fn unspaced(id: &str) -> String {
    id.strip_prefix("minecraft:").unwrap_or(id).to_owned()
}

/// `faces` are the Java model's as the blockstate places it, when Java draws the block.
pub fn rules(base: FluidRules, java: &JavaState, faces: Option<&[BakedFace]>, tags: &Tags) -> FluidRules {
    let blocks_motion = tags.flow.contains(&java.name);
    let solid = blocks_motion || SOLID_UNTAGGED.contains(&java.name.as_str());
    let mut rules = FluidRules { blocks_motion, solid, occludes: !base.ice && !no_occlusion(&java.name), ..base };
    match faces {
        // No geometry: drawn by a block entity (its support shape is the base's) or a bare wall.
        Some([]) => rules.cover = [0.0; 6],
        Some(faces) => {
            rules.cover = std::array::from_fn(|face| cover(faces, face));
            rules.sturdy = rules.cover.map(|c| c >= 1.0);
        }
        None => {}
    }
    let bare_wall = java.name.ends_with("_wall")
        && java.property("up") == Some("false")
        && ["east", "west", "north", "south"].iter().all(|side| java.property(side) == Some("none"));
    if bare_wall {
        rules.sturdy = [false; 6];
    }
    if tags.leaves.contains(&java.name) || NO_SUPPORT.contains(&java.name.as_str()) {
        rules.sturdy = [false; 6];
    }
    // A ladder's support shape is the plate against its wall.
    if let (true, Some(facing)) = (java.name == "ladder", java.property("facing")) {
        let back = ["west", "east", "", "", "north", "south"].iter().position(|f| *f == facing);
        rules.sturdy = std::array::from_fn(|face| Some(face) == back);
    }
    rules
}

/// [`plane_coverage`] of the faces lying in the block's side `face` (east, west, up, down, south, north).
fn cover(faces: &[BakedFace], face: usize) -> f32 {
    let (axis, u, v) = [(0, 2, 1), (0, 2, 1), (1, 0, 2), (1, 0, 2), (2, 0, 1), (2, 0, 1)][face];
    let plane = if face.is_multiple_of(2) { 16.0 } else { 0.0 };
    let rects: Vec<[f64; 4]> = faces
        .iter()
        .filter(|f| f.positions.iter().all(|p| (p[axis] - plane).abs() < 1e-3))
        .map(|f| {
            let span = |i: usize| {
                let values = f.positions.map(|p| f64::from(p[i].clamp(0.0, 16.0)) / 16.0);
                (values.iter().copied().fold(f64::INFINITY, f64::min), values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            };
            let ((u0, u1), (v0, v1)) = (span(u), span(v));
            [u0, u1, v0, v1]
        })
        .collect();
    plane_coverage(&rects, axis == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(positions: [[f32; 3]; 4]) -> BakedFace {
        BakedFace { positions, uvs: [[0.0; 2]; 4], texture: String::new(), cull: None, tint_index: None, shade: None, ambient_occlusion: true }
    }

    #[test]
    fn a_bottom_slab_covers_its_sides_halfway_and_its_bottom() {
        let east = face([[16.0, 8.0, 16.0], [16.0, 0.0, 16.0], [16.0, 0.0, 0.0], [16.0, 8.0, 0.0]]);
        let down = face([[0.0, 0.0, 16.0], [0.0, 0.0, 0.0], [16.0, 0.0, 0.0], [16.0, 0.0, 16.0]]);
        let top = face([[0.0, 8.0, 0.0], [0.0, 8.0, 16.0], [16.0, 8.0, 16.0], [16.0, 8.0, 0.0]]);
        let faces = [east, down, top];
        assert_eq!((cover(&faces, 0), cover(&faces, 1), cover(&faces, 2), cover(&faces, 3)), (0.5, 0.0, 0.0, 1.0));
    }
}
