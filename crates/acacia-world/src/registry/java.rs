//! A registry whose ids are Java 26.3 global palette ids, which are also the neutral state ids. Collision boxes are
//! Java's own; flags, friction, mining and light come from the Bedrock counterpart.
// TODO: selection shapes are still Bedrock's.

use std::sync::{Arc, LazyLock};

use rustc_hash::FxHashMap;

use super::{Aabb, BlockFlags, BlockRegistry, BlockState, custom_state, intern};

static JAVA: LazyLock<Arc<BlockRegistry>> = LazyLock::new(|| {
    let mut registry = BlockRegistry::from_states(java_states());
    registry.ids_are_neutral = true;
    Arc::new(registry)
});

impl BlockRegistry {
    pub fn java() -> &'static BlockRegistry {
        &JAVA
    }

    pub fn java_arc() -> Arc<BlockRegistry> {
        JAVA.clone()
    }
}

fn java_states() -> Vec<BlockState> {
    let bedrock = BlockRegistry::vanilla();
    let mut shapes: FxHashMap<(usize, usize), &'static [Aabb]> = FxHashMap::default();
    (0..).map_while(acacia_registry::BlockState::from_java_id).map(|neutral| {
        let name = neutral.block().name();
        let mut state = bedrock.runtime_id_of(neutral).and_then(|id| bedrock.get(id)).cloned().unwrap_or_else(|| custom_state(name));
        let java = neutral.java_collision().unwrap_or(&[]);
        state.boxes = shapes.entry((java.as_ptr() as usize, java.len())).or_insert_with(|| leak_boxes(java));
        let full = state.boxes == [Aabb::FULL];
        state.flags = BlockFlags(if full { state.flags.0 | BlockFlags::FULL_CUBE.0 } else { state.flags.0 & !BlockFlags::FULL_CUBE.0 });
        state.name = name;
        state.properties = java_properties(neutral);
        state.network_hash = 0;
        state.neutral = Some(neutral);
        state
    }).collect()
}

fn leak_boxes(boxes: &[acacia_registry::CollisionBox]) -> &'static [Aabb] {
    let f = |v: f64| v as f32;
    Box::leak(boxes.iter().map(|b| Aabb::new([f(b[0]), f(b[1]), f(b[2])], [f(b[3]), f(b[4]), f(b[5])])).collect())
}

fn java_properties(state: acacia_registry::BlockState) -> &'static str {
    let mut pairs: Vec<String> = state.properties().map(|(p, v)| format!("{}={v}", p.key())).collect();
    pairs.sort_unstable();
    intern(&pairs.join(","))
}

#[cfg(test)]
mod tests {
    use acacia_registry::{Block, prop, value};

    use super::*;

    #[test]
    fn ids_are_java_state_ids() {
        let java = BlockRegistry::java();
        let slab = Block::OakSlab.default_state().with(prop::SlabType, value::SlabType::Top).unwrap();
        let state = java.get(slab.id()).unwrap();
        assert_eq!(state.name, "minecraft:oak_slab");
        assert_eq!(state.boxes, [Aabb::new([0.0, 0.5, 0.0], [1.0, 1.0, 1.0])]);
        assert_eq!(java.runtime_id_of(slab), Some(slab.id()));
        assert_eq!(java.air_id(), 0);
        assert!(java.get(Block::Stone.default_state().id()).unwrap().flags.contains(BlockFlags::FULL_CUBE));
        assert_eq!(java.get(Block::Ice.default_state().id()).unwrap().friction, 0.98);
    }
}
