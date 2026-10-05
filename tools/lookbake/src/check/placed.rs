//! What depends on where a block is, against the dump's `placement.json`: which alternatives the
//! game picks at sample positions, and how far it shifts the blocks that stand off the grid.

use std::collections::BTreeMap;

use acacia_render::blocks::placed::{Draw, Weighted, seed};
use serde::Deserialize;

use super::Slot;
use crate::blockstate::Drawn;
use crate::offset;

/// Shifts in blocks agree within this.
const TOLERANCE: f64 = 1e-6;

#[derive(Deserialize)]
pub(super) struct Placement {
    positions: Vec<[i32; 3]>,
    /// By block id, the shift at each position.
    offsets: BTreeMap<String, Vec<[f64; 3]>>,
    /// By state key, the parts drawn at each position; only states where that varies.
    pub(super) picks: BTreeMap<String, Vec<Vec<usize>>>,
}

impl Placement {
    /// Whether our draw picks the game's parts for `ours` at every sample position; `theirs` names
    /// each alternative's part.
    pub(super) fn picks_agree(&self, key: &str, ours: &Drawn, theirs: &[Slot]) -> bool {
        let Some(picked) = self.picks.get(key) else { return ours.parts.iter().all(|part| part.len() == 1) };
        let indices: Vec<Weighted<usize>> = ours.parts.iter().map(|p| Weighted(p.iter().enumerate().map(|(i, (weight, _))| (*weight, i)).collect())).collect();
        self.positions.iter().zip(picked).all(|(position, parts)| {
            let draw = Draw::new(seed(*position));
            let draw = if ours.multipart { draw.again() } else { draw };
            let mine = indices.iter().zip(theirs).map(|(list, slot)| list.pick(draw).and_then(|i| slot.get(*i)).map(|a| a.part));
            mine.eq(parts.iter().map(|part| Some(*part)))
        })
    }

    /// Blocks whose shift differs from the game's, or that only one side shifts.
    pub(super) fn offsets_differing(&self) -> Vec<String> {
        let ours = offset::SIDEWAYS.iter().chain(offset::SUNK).chain(offset::NARROW).map(|name| format!("minecraft:{name}"));
        let unknown = ours.filter(|name| !self.offsets.contains_key(name));
        let differs = |(name, shifts): (&String, &Vec<[f64; 3]>)| {
            let Some(offset) = offset::of(name.trim_start_matches("minecraft:")) else { return true };
            self.positions.iter().zip(shifts).any(|(&[x, _, z], theirs)| {
                let mine = offset.at(x, z);
                (0..3).any(|i| (f64::from(mine[i]) - theirs[i]).abs() > TOLERANCE)
            })
        };
        unknown.chain(self.offsets.iter().filter(|entry| differs(*entry)).map(|(name, _)| name.clone())).collect()
    }
}
