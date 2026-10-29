//! What placing a cell takes from the inventory, and a structure's whole bill on an empty site.

use std::collections::BTreeMap;

use super::rules;
use super::structure::{Block, Structure};

/// Items (inventory keys, `name@aux` for coloured beds) that placing `target` uses; empty when unsupported.
pub fn items(target: &Block) -> Vec<(String, u32)> {
    let Some(item) = rules::item(target) else { return Vec::new() };
    let mut out = vec![(item, rules::items_per_cell(target))];
    out.extend(rules::extra_item(target).map(|extra| (extra, 1)));
    out
}

/// Every item the structure's supported cells take, largest count first.
pub fn bill(structure: &Structure) -> Vec<(String, u32)> {
    let mut want: BTreeMap<String, u32> = BTreeMap::new();
    for (item, n) in structure.cells().filter(|(_, b)| !b.is_air()).flat_map(|(_, b)| items(b)) {
        *want.entry(item).or_default() += n;
    }
    let mut out: Vec<_> = want.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}
