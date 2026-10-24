use std::collections::HashMap;
use std::sync::OnceLock;

use super::java;

/// Java biome names by Bedrock biome id; ids Geyser does not map read as plains.
pub struct Biomes {
    names: Vec<&'static str>,
    by_bedrock: HashMap<u32, u16>,
    by_name: HashMap<&'static str, u16>,
    plains: u16,
}

pub fn biomes() -> &'static Biomes {
    static B: OnceLock<Biomes> = OnceLock::new();
    B.get_or_init(|| {
        let list = &java().t.biomes;
        let names: Vec<&'static str> = list.iter().map(|&(_, n)| n).collect();
        let by_bedrock = list
            .iter()
            .enumerate()
            .map(|(i, &(id, _))| (id, i as u16))
            .collect();
        let mut by_name = HashMap::new();
        for (i, n) in names.iter().enumerate() {
            by_name.entry(*n).or_insert(i as u16);
        }
        let plains = by_name["minecraft:plains"];
        Biomes {
            names,
            by_bedrock,
            by_name,
            plains,
        }
    })
}

impl Biomes {
    pub fn java(&self, bedrock_id: u32) -> u16 {
        self.by_bedrock
            .get(&bedrock_id)
            .copied()
            .unwrap_or(self.plains)
    }

    pub fn name(&self, index: u16) -> &'static str {
        self.names[index as usize]
    }

    pub fn index(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }
}
