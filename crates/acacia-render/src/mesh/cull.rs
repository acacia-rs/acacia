//! Whether a face shows against the block next to it.

use super::Ctx;
use super::shapes::neighbour;
use super::smooth::{Dir, OF_FACE};
use crate::blocks::{BlockTable, Cull, Skip};

impl Ctx<'_> {
    /// Whether face `face` (the renderer's axis face) of the block at section-local `p` shows
    /// against its neighbour: Java's `Block.shouldRenderFace` where the look bakes [`Cull`], the
    /// renderer's own rule otherwise.
    pub(super) fn shows(&self, p: [i32; 3], face: usize) -> bool {
        let n = neighbour(p, face);
        let (id, nid) = (self.v.block(p[0], p[1], p[2]), self.v.block(n[0], n[1], n[2]));
        let (own, other) = (self.table.get(id), self.table.get(nid));
        match (own.cull, other.cull) {
            (Some(own), Some(other)) => shows(self.table, own, other, OF_FACE[face]),
            _ => !(other.occludes || (nid == id && own.cull_same)),
        }
    }
}

const EMPTY: [u64; 4] = [0; 4];

/// `Block.shouldRenderFace` of a face towards Java direction `d`.
pub(super) fn shows(table: &BlockTable, own: Cull, other: Cull, d: Dir) -> bool {
    let face = |c: Cull, d: Dir| if c.shape == 0 { EMPTY } else { table.light_shapes[usize::from(c.shape) - 1][usize::from(d)] };
    if other.whole >> (d ^ 1) & 1 == 1 {
        return false;
    }
    if skips(own.skip, other.skip, d) {
        return false;
    }
    let (shape, occluder) = (face(own, d), face(other, d ^ 1));
    if occluder == EMPTY || shape == EMPTY {
        return true;
    }
    shape.iter().zip(occluder).any(|(s, o)| s & !o != 0)
}

/// `skipRendering` of a block with `own` against one with `other` towards `d`.
fn skips(own: Skip, other: Skip, d: Dir) -> bool {
    let vertical = d < 2;
    match (own, other) {
        (Skip::Same { block }, Skip::Same { block: b }) => block == b,
        (Skip::SameVertical { block }, Skip::SameVertical { block: b }) => vertical && block == b,
        (Skip::Bars { block, tagged, sides }, Skip::Bars { block: b, tagged: t, sides: s }) => {
            (block == b || (tagged && t)) && (vertical || (sides >> d & 1 == 1 && s >> (d ^ 1) & 1 == 1))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::FaceShapes;

    const FULL: [u64; 4] = [u64::MAX; 4];
    const UP: Dir = 1;

    fn table(shapes: Vec<FaceShapes>) -> BlockTable {
        let mut table = BlockTable::from_blocks(Vec::new());
        table.light_shapes = shapes.into();
        table
    }

    #[test]
    fn only_a_whole_side_hides_a_shapeless_face() {
        // 1: a full cube's sides; 2: a bottom slab's, full below and empty above; 3: stairs', full
        // below but in two boxes.
        let mut slab = [FULL; 6];
        slab[1] = EMPTY;
        let table = table(vec![[FULL; 6], slab, slab]);
        let cube = Cull { shape: 1, whole: 0b11_1111, skip: Skip::None };
        let bottom_slab = Cull { shape: 2, whole: 0b01, skip: Skip::None };
        let stairs = Cull { shape: 3, whole: 0, skip: Skip::None };
        let glass = Cull { shape: 0, whole: 0, skip: Skip::Same { block: 7 } };
        assert!(shows(&table, glass, stairs, UP), "two boxes over the side do not hide what cannot occlude");
        assert!(!shows(&table, cube, stairs, UP), "but hide a full face");
        assert!(!shows(&table, glass, cube, UP), "a whole block hides anything");
        assert!(!shows(&table, glass, bottom_slab, UP), "a slab's underside is a whole side");
        assert!(shows(&table, cube, bottom_slab, 0), "nothing is against a slab's open top");
        assert!(!shows(&table, glass, glass, UP), "glass leaves out faces against glass");
    }

    #[test]
    fn bars_meet_sideways_only_where_both_connect() {
        let (north, south) = (2u8, 3u8);
        let bars = |sides: u8| Cull { shape: 0, whole: 0, skip: Skip::Bars { block: 1, tagged: true, sides } };
        let table = table(Vec::new());
        assert!(!shows(&table, bars(1 << north), bars(1 << south), north));
        assert!(shows(&table, bars(1 << north), bars(0), north));
        assert!(!shows(&table, bars(0), bars(0), UP));
    }
}
