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
    let poured = rules::redstone::poured(structure.palette.iter());
    structure
        .cells()
        .filter(|(_, b)| b.name != "minecraft:structure_void")
        .filter_map(|(local, b)| {
            let (pos, target) = (at.apply(structure.size, local), at.block(b));
            let seen = block(pos);
            let ok = seen.as_ref().is_some_and(|v| rules::matches(&target, v) || (target.is_air() && v.air) || rules::redstone::spilled(&poured, &target, v));
            let found = seen.map_or("unloaded".to_string(), |v| format!("{} [{}]", v.name, v.props));
            (!ok).then(|| Mismatch { pos, want: format!("{} [{}]", target.name, target.props), found })
        })
        .collect()
}
