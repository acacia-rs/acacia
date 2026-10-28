use acacia_physics::BlockPos;

use super::rules;
use super::structure::Structure;
use super::transform::Transform;
use crate::pathfind::process::BlockView;

/// A cell whose block differs from the structure's.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    pub pos: BlockPos,
    /// `name [props]`.
    pub want: String,
    /// `name [props]`, or `unloaded`.
    pub found: String,
}

/// Compares the world (`block` per world cell) with `structure` placed `at`, as the builder judges a cell:
/// derived states (connections, stair corners) and a double slab's half are not compared.
pub fn check(structure: &Structure, at: &Transform, block: impl Fn(BlockPos) -> Option<BlockView>) -> Vec<Mismatch> {
    structure
        .cells()
        .filter(|(_, b)| b.name != "minecraft:structure_void")
        .filter_map(|(local, b)| {
            let (pos, target) = (at.apply(structure.size, local), at.block(b));
            let seen = block(pos);
            let ok = seen.as_ref().is_some_and(|v| rules::matches(&target, v) || (target.is_air() && v.air));
            let found = seen.map_or("unloaded".to_string(), |v| format!("{} [{}]", v.name, v.props));
            (!ok).then(|| Mismatch { pos, want: format!("{} [{}]", target.name, target.props), found })
        })
        .collect()
}

/// Beds whose colour (`color` per world cell, from its block entity) differs from the structure's, judged at
/// the foot, which holds the block entity.
pub fn colors(structure: &Structure, at: &Transform, color: impl Fn(BlockPos) -> Option<u8>) -> Vec<Mismatch> {
    structure
        .cells()
        .filter_map(|(local, b)| {
            let want = rules::twocell::color(b).filter(|_| b.prop("head_piece_bit") != Some("1"))?;
            let pos = at.apply(structure.size, local);
            let found = color(pos);
            (found != Some(want)).then(|| Mismatch { pos, want: format!("bed color {want}"), found: found.map_or("no color".into(), |c| format!("bed color {c}")) })
        })
        .collect()
}

/// Signs whose text differs from the structure's: `text` gives a world cell's side (front `true`; `None`, as
/// blank: no sign entity known there).
pub fn check_text(structure: &Structure, at: &Transform, text: impl Fn(BlockPos, bool) -> Option<String>) -> Vec<Mismatch> {
    structure
        .cells()
        .filter_map(|(local, _)| {
            let (pos, want) = (at.apply(structure.size, local), structure.text(local)?);
            let found = (text(pos, true).unwrap_or_default(), text(pos, false).unwrap_or_default());
            (found.0 != want.front || found.1 != want.back).then(|| Mismatch { pos, want: format!("text {:?} / {:?}", want.front, want.back), found: format!("text {:?} / {:?}", found.0, found.1) })
        })
        .collect()
}
