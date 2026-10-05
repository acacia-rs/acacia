//! `lookbake java-check`: our bake of every Java block state against the game's own, from a dump
//! of its baked quads (the workspace's `research/java-truth`, whose README has the format).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::blockstate;
use crate::download::Error;
use crate::mapping::JavaState;
use crate::model::bake::{BakedFace, Direction, bake};
use crate::model::{Models, strip_namespace};

/// States named per kind of difference.
const EXAMPLES: usize = 6;
/// Positions in 1/16 block and texture coordinates in texels agree within this.
const TOLERANCE: f32 = 0.01;

#[derive(Deserialize)]
struct Truth {
    parts: Vec<Part>,
    states: BTreeMap<String, State>,
}

#[derive(Deserialize)]
struct State {
    /// Indices into `parts`: the first alternative of each slot.
    parts: Vec<usize>,
}

#[derive(Deserialize)]
struct Part {
    ambient_occlusion: bool,
    quads: Vec<Quad>,
}

#[derive(Deserialize)]
struct Quad {
    /// In blocks.
    positions: [[f32; 3]; 4],
    /// In texels, v downward.
    uvs: [[f32; 2]; 4],
    sprite: String,
    tint_index: i32,
    direction: String,
    shade_direction_override: Option<String>,
    cullface: Option<String>,
}

/// `assets` is the jar's `assets/minecraft`, `truth` the dump's `quads.json.gz`.
pub fn print(assets: &Path, truth: &Path) -> Result<(), Error> {
    let truth: Truth = serde_json::from_reader(flate2::read::GzDecoder::new(File::open(truth)?))?;
    let mut models = Models::new(assets.to_owned());
    let mut blockstates: HashMap<String, Option<Value>> = HashMap::new();
    let mut differences: BTreeMap<&'static str, (usize, Vec<&str>)> = BTreeMap::new();
    let mut blocks: BTreeMap<&'static str, BTreeSet<&str>> = BTreeMap::new();
    let mut same = 0;
    for (key, state) in &truth.states {
        let java = JavaState::parse(key);
        let file = blockstates.entry(java.name.clone()).or_insert_with(|| {
            serde_json::from_slice(&std::fs::read(assets.join(format!("blockstates/{}.json", java.name))).ok()?).ok()
        });
        let placed = file.as_ref().map(|f| blockstate::models(f, &java)).unwrap_or_default();
        let ours: Vec<BakedFace> = placed.iter().flat_map(|p| bake(&models.resolve(&p.model), p)).collect();
        let theirs: Vec<&Quad> = state.parts.iter().flat_map(|&i| &truth.parts[i].quads).collect();
        let ambient_occlusion = state.parts.first().map(|&i| truth.parts[i].ambient_occlusion);
        let found = compare(&ours, &theirs, ambient_occlusion);
        same += usize::from(found.is_empty());
        for difference in found {
            blocks.entry(difference).or_default().insert(key.split('[').next().unwrap_or(key));
            let (count, examples) = differences.entry(difference).or_default();
            *count += 1;
            if examples.len() < EXAMPLES {
                examples.push(key);
            }
        }
    }
    println!("{same} of {} Java block states bake as the game bakes them", truth.states.len());
    for (difference, (count, examples)) in differences {
        println!("  {difference}: {count} states, e.g. {}", examples.join(" "));
        println!("    blocks: {}", blocks[difference].iter().copied().collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

/// How `ours` differs from the game's quads, as kinds of difference.
fn compare(ours: &[BakedFace], theirs: &[&Quad], ambient_occlusion: Option<bool>) -> BTreeSet<&'static str> {
    let mut found = BTreeSet::new();
    let mut left: Vec<&BakedFace> = ours.iter().collect();
    for quad in theirs {
        let positions = quad.positions.map(|p| p.map(|v| v * 16.0));
        let sprite = strip_namespace(&quad.sprite);
        let matches = |textured: bool| {
            left.iter().enumerate().find_map(|(i, f)| Some((i, turn(&f.positions, &positions)?)).filter(|_| !textured || f.texture == sprite))
        };
        let Some((at, turn)) = matches(true).or_else(|| matches(false)) else {
            found.insert("a quad of the game's is missing");
            continue;
        };
        let face = left.swap_remove(at);
        if face.texture != sprite {
            found.insert("texture");
        }
        if (0..4).any(|i| !near(&face.uvs[(i + turn) % 4], &quad.uvs[i])) {
            found.insert("texture coordinates");
        }
        if face.cull != quad.cullface.as_deref().and_then(Direction::named) {
            found.insert("cull side");
        }
        if face.tinted != (quad.tint_index >= 0) {
            found.insert("tinted or not");
        }
        let shaded_as = quad.shade_direction_override.as_deref().unwrap_or(&quad.direction);
        if face.shade != Direction::named(shaded_as).filter(|d| quad.shade_direction_override.is_none() || *d != Direction::Up) {
            found.insert("shade direction");
        }
    }
    if !left.is_empty() {
        found.insert("a quad the game does not have");
    }
    if ours.first().zip(ambient_occlusion).is_some_and(|(face, theirs)| face.ambient_occlusion != theirs) {
        found.insert("ambient occlusion");
    }
    found
}

fn near<const N: usize>(a: &[f32; N], b: &[f32; N]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < TOLERANCE)
}

/// How many vertices on `ours` starts from `theirs`' first, when they are the same quad wound the same way.
fn turn(ours: &[[f32; 3]; 4], theirs: &[[f32; 3]; 4]) -> Option<usize> {
    (0..4).find(|turn| (0..4).all(|i| near(&ours[(i + turn) % 4], &theirs[i])))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [[f32; 3]; 4] = [[0.0, 16.0, 0.0], [0.0, 16.0, 16.0], [16.0, 16.0, 16.0], [16.0, 16.0, 0.0]];
    const UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 16.0], [16.0, 16.0], [16.0, 0.0]];

    fn ours() -> BakedFace {
        let (cull, shade) = (Some(Direction::Up), Some(Direction::Up));
        BakedFace { positions: SQUARE, uvs: UVS, texture: "block/stone".into(), cull, tinted: false, shade, ambient_occlusion: true }
    }

    fn theirs(start: usize) -> Quad {
        Quad {
            positions: std::array::from_fn(|i| SQUARE[(i + start) % 4].map(|v| v / 16.0)),
            uvs: std::array::from_fn(|i| UVS[(i + start) % 4]),
            sprite: "minecraft:block/stone".into(),
            tint_index: -1,
            direction: "up".into(),
            shade_direction_override: None,
            cullface: Some("up".into()),
        }
    }

    #[test]
    fn the_same_quad_matches_from_any_first_vertex() {
        assert!(compare(&[ours()], &[&theirs(0)], Some(true)).is_empty());
        assert!(compare(&[ours()], &[&theirs(3)], Some(true)).is_empty());
    }

    #[test]
    fn differences_are_named() {
        let mut quad = theirs(0);
        (quad.uvs[0], quad.tint_index, quad.cullface) = ([1.0, 0.0], 0, None);
        let found = compare(&[ours()], &[&quad], Some(false));
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["ambient occlusion", "cull side", "texture coordinates", "tinted or not"]);
        let mut reversed = theirs(0);
        reversed.positions.reverse();
        let found = compare(&[ours()], &[&reversed], Some(true));
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["a quad of the game's is missing", "a quad the game does not have"]);
    }
}
