//! Java collision boxes per state, dumped from the 26.3 jar with an empty context at the origin. Blocks whose shape
//! depends on the entity or on neighbours (scaffolding, powder snow) give that context's shape here.

use std::sync::OnceLock;

use crate::state::BlockState;

/// `[min x, min y, min z, max x, max y, max z]`, block-local.
pub type CollisionBox = [f64; 6];

static DATA: &[u8] = include_bytes!("generated/collision.bin");

struct Table {
    state_shape: Vec<u16>,
    first_box: Vec<u32>,
    boxes: Vec<CollisionBox>,
}

fn u32_at(at: usize) -> usize {
    u32::from_le_bytes(DATA[at..at + 4].try_into().expect("four bytes")) as usize
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let states = u32_at(0);
        let state_shape = DATA[4..4 + states * 2].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        let shapes_at = 4 + states * 2;
        let shapes = u32_at(shapes_at);
        let first_box: Vec<u32> = (0..=shapes).map(|i| u32_at(shapes_at + 4 + i * 4) as u32).collect();
        let boxes_at = shapes_at + 4 + (shapes + 1) * 4;
        let boxes = DATA[boxes_at..]
            .chunks_exact(48)
            .map(|b| std::array::from_fn(|i| f64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().expect("eight bytes"))))
            .collect();
        Table { state_shape, first_box, boxes }
    })
}

impl BlockState {
    /// The state's Java collision boxes; `None` for Bedrock-only states.
    pub fn java_collision(self) -> Option<&'static [CollisionBox]> {
        let t = table();
        let shape = *t.state_shape.get(self.id() as usize)? as usize;
        Some(&t.boxes[t.first_box[shape] as usize..t.first_box[shape + 1] as usize])
    }
}

#[cfg(test)]
mod tests {
    use crate::prop;
    use crate::value;
    use crate::Block;

    #[test]
    fn known_shapes() {
        assert_eq!(Block::Air.default_state().java_collision(), Some(&[][..]));
        assert_eq!(Block::Stone.default_state().java_collision(), Some(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]][..]));
        let slab = Block::OakSlab.default_state().with(prop::SlabType, value::SlabType::Top).unwrap();
        assert_eq!(slab.java_collision(), Some(&[[0.0, 0.5, 0.0, 1.0, 1.0, 1.0]][..]));
    }

    #[test]
    fn every_java_state_has_a_shape() {
        let last = crate::BlockState::from_java_id(0).unwrap();
        assert!(last.java_collision().is_some());
        let mut id = 0;
        while let Some(s) = crate::BlockState::from_java_id(id) {
            assert!(s.java_collision().is_some(), "{id}");
            id += 1;
        }
        assert!(id > 30_000);
    }
}
