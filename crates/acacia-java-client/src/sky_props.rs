//! What the sky light engine reads from a block state, dumped from 26.3 by `tools/SkyLightDump.java`.

use std::sync::OnceLock;

static DATA: &[u8] = include_bytes!("../data/sky_light.bin");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    dampening: u8,
    use_shape: bool,
    emission: u8,
    /// Classes of `LightEngine.getOcclusionShape` for DOWN and UP.
    down: u8,
    up: u8,
}

struct Table {
    classes: usize,
    occludes: &'static [u8],
    rows: Vec<Row>,
    row_of: &'static [u8],
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let classes = usize::from(DATA[0]);
        let (occludes, rest) = DATA[1..].split_at(classes * classes);
        let (rows, rest) = rest[1..].split_at(usize::from(rest[0]) * 4);
        let rows = rows.chunks(4).map(|r| Row { dampening: r[0] & 15, use_shape: r[0] & 16 != 0, emission: r[1], down: r[2], up: r[3] }).collect();
        let count = u32::from_be_bytes(rest[..4].try_into().expect("state count")) as usize;
        Table { classes, occludes, rows, row_of: &rest[4..4 + count] }
    })
}

/// Air's row for ids past Java's palette.
fn row(state: u32) -> Row {
    let t = table();
    t.rows[usize::from(t.row_of.get(state as usize).copied().unwrap_or(t.row_of[0]))]
}

/// `ChunkSkyLightSources.isEdgeOccluded`: sky light can't pass down from `top` into `bottom`.
pub(crate) fn edge_occluded(top: u32, bottom: u32) -> bool {
    let (t, top, bottom) = (table(), row(top), row(bottom));
    bottom.dampening != 0 || t.occludes[usize::from(top.down) * t.classes + usize::from(bottom.up)] != 0
}

/// `LightEngine.hasDifferentLightProperties`.
pub(crate) fn light_properties_differ(old: u32, new: u32) -> bool {
    let (o, n) = (row(old), row(new));
    old != new && (o.dampening != n.dampening || o.emission != n.emission || o.use_shape || n.use_shape)
}

#[cfg(test)]
mod tests {
    use acacia_registry::{Block, BlockState, prop, value};

    use super::*;

    fn id(block: Block) -> u32 {
        block.default_state().id()
    }

    #[test]
    fn covers_every_java_state() {
        assert_eq!(table().row_of.len(), (0..).take_while(|&i| BlockState::from_java_id(i).is_some()).count());
    }

    #[test]
    fn sky_passes_air_and_glass_but_not_stone_leaves_or_a_bottom_slab() {
        let air = id(Block::Air);
        assert!(!edge_occluded(air, air) && !edge_occluded(air, id(Block::Glass)));
        assert!(edge_occluded(air, id(Block::Stone)) && edge_occluded(air, id(Block::OakLeaves)));
        let top_slab = Block::StoneSlab.default_state().with(prop::SlabType, value::SlabType::Top);
        // A top slab's lower face is open, but the air above a bottom slab still meets its full lower face below.
        assert!(edge_occluded(id(Block::StoneSlab), air), "a bottom slab's full DOWN face");
        assert!(!edge_occluded(top_slab.map_or(air, BlockState::id), air), "a top slab's open DOWN face");
        assert!(light_properties_differ(air, id(Block::Stone)) && !light_properties_differ(air, id(Block::Glass)));
    }
}
