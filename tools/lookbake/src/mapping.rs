//! Bedrock block state to Java block state, from ViaBedrock's table. The table is GPL-3.0: it is
//! downloaded when baking and never vendored. Why this table: docs/java-look.md.

use std::collections::HashMap;
use std::path::Path;

use crate::download::{Error, get, pinned};

const COMMIT: &str = "6e242b278740ceccac20e5db0915b00775095b24";
const FILE: &str = "src/main/resources/assets/viabedrock/data/custom/blockstate_mappings.json";
const SHA1: &str = "1f93fd312bcc4117162a704a4e716cc3dabcfd75";

/// A Java block state: `oak_log` with `axis=y`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaState {
    /// Without the `minecraft:` namespace, as blockstate files are named.
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl JavaState {
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// From `minecraft:oak_log[axis=y]` or a bare name.
    fn parse(text: &str) -> JavaState {
        let (name, properties) = text.split_once('[').map_or((text, ""), |(n, p)| (n, p.trim_end_matches(']')));
        let pairs = properties.split(',').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned()));
        JavaState { name: name.strip_prefix("minecraft:").unwrap_or(name).to_owned(), properties: pairs.collect() }
    }
}

/// The table lists `minecraft:`-prefixed properties first; the registry sorts all of them by key.
fn sorted(bedrock_state: &str) -> String {
    let Some((name, properties)) = bedrock_state.split_once('[') else { return bedrock_state.to_owned() };
    let mut pairs: Vec<&str> = properties.trim_end_matches(']').split(',').collect();
    pairs.sort_unstable_by_key(|p| p.split_once('=').map_or(*p, |(key, _)| key));
    format!("{name}[{}]", pairs.join(","))
}

/// By Bedrock state, spelled as `acacia_render::lookpack::state_key` spells it.
pub struct Mapping(HashMap<String, JavaState>);

impl Mapping {
    /// Downloads the table into `dir` unless it is there already.
    pub fn fetch(dir: &Path) -> Result<Mapping, Error> {
        let url = format!("https://raw.githubusercontent.com/RaphiMC/ViaBedrock/{COMMIT}/{FILE}");
        Mapping::parse(&pinned(&dir.join("blockstate_mappings.json"), SHA1, || get(&url))?)
    }

    fn parse(json: &[u8]) -> Result<Mapping, Error> {
        let table: HashMap<String, String> = serde_json::from_slice(json)?;
        Ok(Mapping(table.into_iter().map(|(bedrock, java)| (sorted(&bedrock), JavaState::parse(&java))).collect()))
    }

    pub fn get(&self, bedrock_state: &str) -> Option<&JavaState> {
        self.0.get(bedrock_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_parse_with_and_without_properties() {
        let json = br#"{"minecraft:oak_log[pillar_axis=y]": "minecraft:oak_log[axis=y]", "minecraft:stone": "minecraft:stone"}"#;
        let mapping = Mapping::parse(json).unwrap();
        let log = mapping.get("minecraft:oak_log[pillar_axis=y]").unwrap();
        assert_eq!((log.name.as_str(), log.property("axis"), log.property("half")), ("oak_log", Some("y"), None));
        assert_eq!(mapping.get("minecraft:stone"), Some(&JavaState { name: "stone".into(), properties: Vec::new() }));
        assert_eq!(mapping.get("minecraft:dirt"), None);
    }

    #[test]
    fn bedrock_properties_are_looked_up_sorted_by_key() {
        let json = br#"{"minecraft:leaf_litter[minecraft:cardinal_direction=south,growth=0]": "minecraft:leaf_litter[facing=south,segment_amount=1]"}"#;
        let mapping = Mapping::parse(json).unwrap();
        assert!(mapping.get("minecraft:leaf_litter[growth=0,minecraft:cardinal_direction=south]").is_some());
    }
}
