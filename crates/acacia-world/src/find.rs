//! Finding blocks by name around a point, nearest first.

use std::collections::{BinaryHeap, HashSet};

use crate::{BlockRegistry, SECTION_VOLUME, World};

/// Every runtime id (all states) of the blocks named; a name without a namespace is `minecraft:`'s.
pub fn block_ids<S: AsRef<str>>(registry: &BlockRegistry, names: &[S]) -> HashSet<u32> {
    let full = |n: &str| if n.contains(':') { n.to_owned() } else { format!("minecraft:{n}") };
    names.iter().map(|n| full(n.as_ref())).flat_map(|n| registry.states_of(&n).map(|(id, _)| id).collect::<Vec<_>>()).collect()
}

/// Up to `count` cells within `max_distance` blocks (Euclidean, cell to cell) of `center` whose layer-0 block is one
/// of `ids`, nearest first (ties in x, y, z order). Only loaded chunks count; sections whose palette holds none of
/// `ids` are skipped without being read.
pub fn find_blocks(world: &World, center: [i32; 3], ids: &HashSet<u32>, max_distance: u32, count: usize) -> Vec<[i32; 3]> {
    if ids.is_empty() || count == 0 {
        return Vec::new();
    }
    let r = max_distance as i32;
    let r2 = i64::from(r) * i64::from(r);
    let dist2 = |p: [i32; 3]| p.iter().zip(center).map(|(a, c)| i64::from(a - c).pow(2)).sum::<i64>();
    // The `count` nearest so far, farthest on top.
    let mut found = BinaryHeap::new();
    for cx in (center[0] - r) >> 4..=(center[0] + r) >> 4 {
        for cz in (center[2] - r) >> 4..=(center[2] + r) >> 4 {
            let Some(shared) = world.get(cx, cz) else { continue };
            let chunk = shared.read();
            let min_y = chunk.dimension().min_y;
            for index in 0..chunk.section_count() {
                let base = min_y + 16 * index as i32;
                if base + 15 < center[1] - r || base > center[1] + r || !chunk.section_palette(index).iter().any(|id| ids.contains(id)) {
                    continue;
                }
                for i in 0..SECTION_VOLUME as i32 {
                    let p = [cx * 16 + (i >> 8), base + (i & 15), cz * 16 + ((i >> 4) & 15)];
                    let key = (dist2(p), p);
                    let better = found.len() < count || found.peek().is_some_and(|worst| key < *worst);
                    if key.0 <= r2 && better && ids.contains(&chunk.block(p[0], p[1], p[2])) {
                        found.push(key);
                        if found.len() > count {
                            found.pop();
                        }
                    }
                }
            }
        }
    }
    found.into_sorted_vec().into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockIds;

    #[test]
    fn nearest_matches_within_range_and_count() {
        let registry = BlockRegistry::vanilla_arc();
        let world = World::new(registry.clone(), 0, BlockIds::Runtime);
        let _held: Vec<_> = [(0, 0), (-1, 0), (5, 5)].map(|(x, z)| world.insert_sub_chunk_air(x, 4, z)).into();
        let stone = block_ids(&registry, &["stone"]);
        let diamond = block_ids(&registry, &["minecraft:diamond_ore", "deepslate_diamond_ore"]);
        let id = |ids: &HashSet<u32>| *ids.iter().next().unwrap();
        let cells = [[3, 64, 0], [-2, 64, 1], [1, 65, 1], [80, 70, 80]];
        for c in cells {
            world.set_block(c[0], c[1], c[2], 0, id(&stone));
        }
        world.set_block(2, 64, 2, 0, id(&diamond));

        assert_eq!(find_blocks(&world, [0, 64, 0], &stone, 16, 10), [[1, 65, 1], [-2, 64, 1], [3, 64, 0]]);
        assert_eq!(find_blocks(&world, [0, 64, 0], &stone, 16, 2), [[1, 65, 1], [-2, 64, 1]]);
        assert_eq!(find_blocks(&world, [0, 64, 0], &stone, 2, 10), [[1, 65, 1]]);
        assert_eq!(find_blocks(&world, [0, 64, 0], &diamond, 16, 10), [[2, 64, 2]]);
        assert!(diamond.len() == 2 && stone.len() == 1, "every state of each name");
        assert!(find_blocks(&world, [0, 64, 0], &block_ids(&registry, &["no_such_block"]), 16, 10).is_empty());
    }
}
