//! How much of the Bedrock palette the Java assets cover: the measure of a Java look's fallbacks.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use acacia_render::lookpack::state_key;
use acacia_world::BlockRegistry;
use serde_json::Value;

use crate::blockstate;
use crate::download::Error;
use crate::mapping::Mapping;

/// Examples printed per kind of miss.
const SHOWN: usize = 8;

#[derive(Default)]
struct Misses {
    count: usize,
    examples: BTreeSet<String>,
}

impl Misses {
    fn add(&mut self, example: &str) {
        self.count += 1;
        if self.examples.len() < SHOWN {
            self.examples.insert(example.to_owned());
        }
    }
}

/// Prints, for the vanilla registry, how many states reach a Java model file. `assets` is the
/// jar's `assets/minecraft`.
pub fn print(assets: &Path, mapping: &Mapping) -> Result<(), Error> {
    let registry = BlockRegistry::vanilla();
    let mut blockstates: HashMap<String, Option<Value>> = HashMap::new();
    let (mut unmapped, mut no_blockstate, mut no_model, mut no_file) = (Misses::default(), Misses::default(), Misses::default(), Misses::default());
    let (mut drawn, mut models) = (0, BTreeSet::new());
    for id in 0..registry.len() as u32 {
        let key = state_key(registry.get(id).expect("id below len"));
        let Some(java) = mapping.get(&key) else {
            unmapped.add(&key);
            continue;
        };
        let file = blockstates.entry(java.name.clone()).or_insert_with(|| {
            let text = std::fs::read(assets.join(format!("blockstates/{}.json", java.name))).ok()?;
            serde_json::from_slice(&text).ok()
        });
        let Some(file) = file else {
            no_blockstate.add(&java.name);
            continue;
        };
        let refs: Vec<_> = blockstate::drawn(file, java).parts.into_iter().flatten().map(|(_, model)| model).collect();
        if refs.is_empty() {
            no_model.add(&key);
            continue;
        }
        drawn += 1;
        for model in refs {
            if !assets.join(format!("models/{}.json", model.model)).is_file() {
                no_file.add(&model.model);
            }
            models.insert(model.model);
        }
    }
    println!("{} Bedrock states: {drawn} draw {} Java models", registry.len(), models.len());
    for (what, misses) in [("not in the mapping", unmapped), ("Java block has no blockstate file", no_blockstate), ("no variant or part matches", no_model), ("model file missing", no_file)] {
        println!("  {what}: {} {:?}", misses.count, misses.examples);
    }
    Ok(())
}
