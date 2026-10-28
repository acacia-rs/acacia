//! A block's selection boxes where it stands: `BlockState::selection` shifted by its random offset, and the
//! shapes BDS works out from neighbours (doors, vines, scaffolding...). Rules and evidence: docs/selection-shapes.md.

use crate::{Aabb, BlockAccess, BlockRegistry, BlockState};

const fn b(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb { min, max }
}

/// A door's 0.1825 slab per side (BDS table 14a941ab0), indexed by `door_slab`.
const DOOR: [Aabb; 4] = [
    b([0.0; 3], [1.0, 1.0, 0.1825]),
    b([0.8175, 0.0, 0.0], [1.0; 3]),
    b([0.0, 0.0, 0.8175], [1.0; 3]),
    b([0.0; 3], [0.1825, 1.0, 1.0]),
];
const FULL: Aabb = b([0.0; 3], [1.0; 3]);
const VINE_COLUMN: Aabb = b([0.0625, 0.0, 0.0625], [0.9375, 1.0, 0.9375]);
const PORTAL_X: Aabb = b([0.0, 0.0, 0.375], [1.0, 1.0, 0.625]);
const PORTAL_Z: Aabb = b([0.375, 0.0, 0.0], [0.625, 1.0, 1.0]);
/// The top slab and posts; the four bottom beams follow in the table.
const SCAFFOLD_TOP_BOXES: usize = 5;

/// The selection boxes of the block at `pos`, block-local.
pub fn boxes_at(world: &impl BlockAccess, registry: &BlockRegistry, pos: [i32; 3]) -> Vec<Aabb> {
    let Some(state) = registry.get(world.block(pos[0], pos[1], pos[2])) else { return Vec::new() };
    let at = |dx: i32, dy: i32, dz: i32| registry.get(world.block(pos[0] + dx, pos[1] + dy, pos[2] + dz));
    let name = state.name.trim_start_matches("minecraft:");
    let shape: Vec<Aabb> = if name.ends_with("_door") || name == "wooden_door" {
        vec![door(state, at(0, 1, 0), at(0, -1, 0))]
    } else if name == "vine" && state.property("vine_direction_bits") == Some("0") {
        vine_without_faces(at(0, 1, 0), at(0, -1, 0))
    } else if name == "twisting_vines" && at(0, 1, 0).is_some_and(|s| s.name == state.name) {
        vec![VINE_COLUMN]
    } else if name == "weeping_vines" && at(0, -1, 0).is_some_and(|s| s.name == state.name) {
        vec![VINE_COLUMN]
    } else if name == "chorus_plant" {
        vec![chorus(|d: [i32; 3]| at(d[0], d[1], d[2]))]
    } else if name == "scaffolding" && at(0, -1, 0).is_some_and(supports_scaffolding) {
        state.selection[..SCAFFOLD_TOP_BOXES.min(state.selection.len())].to_vec()
    } else if name == "portal" && state.property("portal_axis") == Some("unknown") {
        let same = |dx| at(dx, 0, 0).is_some_and(|s| s.name == state.name);
        vec![if same(-1) || same(1) { PORTAL_X } else { PORTAL_Z }]
    } else {
        state.selection.to_vec()
    };
    let d = state.selection_offset.at(pos[0], pos[2]);
    shape.into_iter().map(|a| b(std::array::from_fn(|i| a.min[i] + d[i]), std::array::from_fn(|i| a.max[i] + d[i]))).collect()
}

fn flag(s: &BlockState, key: &str) -> bool {
    matches!(s.property(key), Some("1" | "true"))
}

/// Direction and open from the lower half, hinge from the upper; a half without its pair reads all three as 0.
fn door(state: &BlockState, above: Option<&BlockState>, below: Option<&BlockState>) -> Aabb {
    let upper = flag(state, "upper_block_bit");
    let other = if upper { below } else { above }.filter(|s| s.name == state.name);
    let Some(other) = other else { return door_slab(0, false, false) };
    let (lower, top) = if upper { (other, state) } else { (state, other) };
    let dir = match lower.property("minecraft:cardinal_direction") {
        Some("west") => 1,
        Some("north") => 2,
        Some("east") => 3,
        _ => 0,
    };
    door_slab(dir, flag(lower, "open_bit"), flag(top, "door_hinge_bit"))
}

fn door_slab(dir: usize, open: bool, hinge: bool) -> Aabb {
    let h = usize::from(hinge);
    let i = if open { [h * 2, h * 2 + 1, (h ^ 1) * 2, (h ^ 1) * 2 + 1][dir] } else { [3, 0, 1, 2][dir] };
    DOOR[i]
}

/// A vine with no face bits hangs from or stands on what qualifies above and below; neither leaves the cube.
fn vine_without_faces(above: Option<&BlockState>, below: Option<&BlockState>) -> Vec<Aabb> {
    // BDS: CubeShaped, blocks motion, not invisible_bedrock, and not one more block (global 14ccf9950, unknown).
    let qualifies = |s: Option<&BlockState>| s.is_some_and(|s| s.is_full_cube() && s.name != "minecraft:invisible_bedrock");
    match (qualifies(above), qualifies(below)) {
        (true, false) => vec![b([0.0, 0.9375, 0.0], [1.0; 3])],
        (false, true) => vec![b([0.0; 3], [1.0, 0.0625, 1.0])],
        _ => vec![FULL],
    }
}

/// The 2/16..14/16 core, each side out to the edge where a chorus plant or flower joins (end stone below too).
fn chorus<'a>(at: impl Fn([i32; 3]) -> Option<&'a BlockState>) -> Aabb {
    let joins = |d: [i32; 3]| {
        at(d).is_some_and(|s| {
            s.name == "minecraft:chorus_plant" || s.name == "minecraft:chorus_flower" || (d[1] == -1 && s.name == "minecraft:end_stone")
        })
    };
    let lo = |d| if joins(d) { 0.0 } else { 0.125 };
    let hi = |d| if joins(d) { 1.0 } else { 0.875 };
    b([lo([-1, 0, 0]), lo([0, -1, 0]), lo([0, 0, -1])], [hi([1, 0, 0]), hi([0, 1, 0]), hi([0, 0, 1])])
}

/// BDS asks the block below for edge support on its top face; approximated as scaffolding below or a full top.
fn supports_scaffolding(s: &BlockState) -> bool {
    s.name == "minecraft:scaffolding"
        || s.boxes.iter().any(|a| a.max[1] >= 1.0 && a.min[0] <= 0.0 && a.min[2] <= 0.0 && a.max[0] >= 1.0 && a.max[2] >= 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Cells(HashMap<[i32; 3], u32>);

    impl BlockAccess for Cells {
        fn block(&self, x: i32, y: i32, z: i32) -> u32 {
            self.0.get(&[x, y, z]).copied().unwrap_or_else(|| BlockRegistry::vanilla().air_id())
        }
        fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
            BlockRegistry::vanilla().air_id()
        }
    }

    fn id(name: &str, props: &str) -> u32 {
        BlockRegistry::vanilla().find(name, props).unwrap_or_else(|| panic!("{name} {props}"))
    }

    fn shape(cells: &[([i32; 3], u32)], pos: [i32; 3]) -> Vec<Aabb> {
        boxes_at(&Cells(cells.iter().copied().collect()), BlockRegistry::vanilla(), pos)
    }

    fn first(name: &str) -> u32 {
        BlockRegistry::vanilla().states_of(name).next().expect(name).0
    }

    fn door_state(upper: bool, open: bool, hinge: bool) -> u32 {
        let props = format!(
            "door_hinge_bit={},minecraft:cardinal_direction=north,open_bit={},upper_block_bit={}",
            u8::from(hinge),
            u8::from(open),
            u8::from(upper)
        );
        id("minecraft:wooden_door", &props)
    }

    #[test]
    fn a_door_reads_open_from_its_lower_half_and_hinge_from_its_upper() {
        let (lower, upper) = (door_state(false, true, false), door_state(true, false, true));
        let pair = [([0, 0, 0], lower), ([0, 1, 0], upper)];
        let expected = door_slab(2, true, true);
        assert_eq!(shape(&pair, [0, 0, 0]), vec![expected]);
        assert_eq!(shape(&pair, [0, 1, 0]), vec![expected]);
        assert_eq!(shape(&[([0, 0, 0], lower)], [0, 0, 0]), vec![door_slab(0, false, false)]);
    }

    #[test]
    fn neighbours_shape_vines_chorus_and_scaffolding() {
        let stone = id("minecraft:stone", "");
        let vine = id("minecraft:vine", "vine_direction_bits=0");
        assert_eq!(shape(&[([0, 0, 0], vine), ([0, 1, 0], stone)], [0, 0, 0]), vec![b([0.0, 0.9375, 0.0], [1.0; 3])]);
        assert_eq!(shape(&[([0, 0, 0], vine)], [0, 0, 0]), vec![FULL]);

        let chorus = id("minecraft:chorus_plant", "");
        let got = shape(&[([0, 0, 0], chorus), ([0, -1, 0], id("minecraft:end_stone", "")), ([1, 0, 0], chorus)], [0, 0, 0]);
        assert_eq!(got, vec![b([0.125, 0.0, 0.125], [1.0, 0.875, 0.875])]);

        let scaffold = first("minecraft:scaffolding");
        assert_eq!(shape(&[([0, 0, 0], scaffold)], [0, 0, 0]).len(), 9);
        assert_eq!(shape(&[([0, 0, 0], scaffold), ([0, -1, 0], stone)], [0, 0, 0]).len(), SCAFFOLD_TOP_BOXES);
    }

    #[test]
    fn plants_carry_their_random_offset() {
        let poppy = first("minecraft:poppy");
        let base = BlockRegistry::vanilla().get(poppy).expect("poppy").selection[0];
        let d = crate::SelectionOffset::Xz.at(5, 9);
        let got = shape(&[([5, 64, 9], poppy)], [5, 64, 9]);
        assert_eq!(got, vec![b(std::array::from_fn(|i| base.min[i] + d[i]), std::array::from_fn(|i| base.max[i] + d[i]))]);
    }
}
