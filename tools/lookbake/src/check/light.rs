//! The Java look's light facts (light.rs) of every mapped Bedrock state against the game's, from
//! the `facts.json` of tools/parity harness `light_facts`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use acacia_render::LookPack;
use acacia_render::blocks::{FaceShapes, Skip};
use acacia_render::lookpack::state_key;
use acacia_world::BlockRegistry;
use serde::Deserialize;

use super::Outcome;
use super::shade::dump_key;
use crate::download::Error;
use crate::mapping::Mapping;
use crate::occlusion::grid;

#[derive(Deserialize)]
struct Facts {
    emission: u8,
    dampening: u8,
    solid_render: bool,
    full_collision: bool,
    emissive: bool,
    dark: bool,
    faces: Option<Vec<String>>,
    occlusion: Option<Vec<String>>,
    #[serde(default)]
    occlusion_whole: Vec<bool>,
    skip: Option<String>,
}


pub fn print(look: &LookPack, mapping: &Mapping, dump: &Path) -> Result<Outcome, Error> {
    let truth: HashMap<String, Facts> = serde_json::from_slice(&std::fs::read(dump)?)?;
    let registry = BlockRegistry::vanilla();
    let mut outcome = Outcome::default();
    let mut by_field: BTreeMap<&'static str, BTreeMap<String, usize>> = BTreeMap::new();
    for state in (0..registry.len() as u32).filter_map(|id| registry.get(id)) {
        let key = state_key(state);
        let (Some(java), Some(block)) = (mapping.get(&key), look.block(&key)) else { continue };
        let Some(theirs) = truth.get(&dump_key(java)) else { continue };
        outcome.cases += 1;
        let ours = block.light;
        let our_faces: Option<FaceShapes> = (ours.shape != 0).then(|| look.light_shapes()[usize::from(ours.shape) - 1]);
        let their_faces: Option<FaceShapes> = theirs.faces.as_ref().map(|f| std::array::from_fn(|d| grid(&f[d])));
        let cull = block.cull.unwrap_or_default();
        let our_occlusion: Option<FaceShapes> = (cull.shape != 0).then(|| look.light_shapes()[usize::from(cull.shape) - 1]);
        let their_occlusion: Option<FaceShapes> = theirs.occlusion.as_ref().map(|f| std::array::from_fn(|d| grid(&f[d])));
        let our_skip = match cull.skip {
            Skip::None => None,
            Skip::Same { .. } => Some("same"),
            Skip::SameVertical { .. } => Some("same_vertical"),
            Skip::Bars { tagged: true, .. } => Some("bars_tagged"),
            Skip::Bars { .. } => Some("bars"),
        };
        let fields: [(&'static str, String, String); 10] = [
            ("emission", theirs.emission.to_string(), ours.emission.to_string()),
            ("dampening", theirs.dampening.to_string(), ours.dampening.to_string()),
            ("solid render", theirs.solid_render.to_string(), ours.solid_render.to_string()),
            ("full collision", theirs.full_collision.to_string(), ours.full_collision.to_string()),
            ("emissive", theirs.emissive.to_string(), ours.emissive.to_string()),
            ("darkens", theirs.dark.to_string(), ours.dark.to_string()),
            ("light shape", format!("{their_faces:?}"), format!("{our_faces:?}")),
            ("occlusion shape", format!("{their_occlusion:?}"), format!("{our_occlusion:?}")),
            ("whole-block faces", format!("{:?}", theirs.occlusion_whole), format!("{:?}", (0..6).filter(|_| cull.shape != 0).map(|d| cull.whole >> d & 1 == 1).collect::<Vec<_>>())),
            ("skip rendering", format!("{:?}", theirs.skip), format!("{:?}", our_skip.map(str::to_owned))),
        ];
        for (field, expected, got) in fields {
            if expected != got {
                *by_field.entry(field).or_default().entry(java.name.clone()).or_default() += 1;
                let short = |s: &str| s.chars().take(80).collect::<String>();
                outcome.diverge(&key, field, short(&expected), short(&got));
            }
        }
    }
    println!("{} mapped states; light facts differing:", outcome.cases);
    for (field, blocks) in &by_field {
        let names: Vec<String> = blocks.iter().map(|(name, n)| format!("{name} ({n})")).collect();
        println!("  {field}: {}", names.join(" "));
    }
    Ok(outcome)
}
