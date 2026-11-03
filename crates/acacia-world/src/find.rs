//! Finding blocks by name around a point, nearest first.

use std::collections::{BinaryHeap, HashSet};

use crate::{BlockRegistry, World};

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
    // The part of a 16-cell span starting at `base` inside the search box on `axis`.
    let clip = |axis: usize, base: i32| (base.max(center[axis] - r), (base + 15).min(center[axis] + r));
    for cx in (center[0] - r) >> 4..=(center[0] + r) >> 4 {
        for cz in (center[2] - r) >> 4..=(center[2] + r) >> 4 {
            let Some(shared) = world.get(cx, cz) else { continue };
            let chunk = shared.read();
            let min_y = chunk.dimension().min_y;
            let ((x0, x1), (z0, z1)) = (clip(0, cx * 16), clip(2, cz * 16));
            for index in 0..chunk.section_count() {
                let (y0, y1) = clip(1, min_y + 16 * index as i32);
                if y0 > y1 || !chunk.section_palette(index).iter().any(|id| ids.contains(id)) {
                    continue;
                }
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        for z in z0..=z1 {
                            let p = [x, y, z];
                            let key = (dist2(p), p);
                            let better = found.len() < count || found.peek().is_some_and(|worst| key < *worst);
                            if key.0 <= r2 && better && ids.contains(&chunk.block(x, y, z)) {
                                found.push(key);
                                if found.len() > count {
                                    found.pop();
                                }
                            }
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

    #[test]
    fn matches_a_brute_force_scan_on_a_random_world() {
        let registry = BlockRegistry::vanilla_arc();
        let world = World::new(registry.clone(), 0, BlockIds::Runtime);
        let chunks: Vec<(i32, i32)> = (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))).collect();
        let _held: Vec<_> = chunks.iter().flat_map(|&(x, z)| [4, 5].map(|s| world.insert_sub_chunk_air(x, s, z))).collect();
        let (stone, dirt) = (block_ids(&registry, &["stone"]), block_ids(&registry, &["dirt"]));
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n) as i32
        };
        let cells = |x: i32, z: i32| (-16..32).flat_map(move |dx| (64..96).flat_map(move |y| (-16..32).map(move |dz| [x + dx, y, z + dz])));
        for p in cells(0, 0) {
            let ids = match next(40) {
                0 => &stone,
                1 if p[1] < 80 => &dirt,
                _ => continue,
            };
            world.set_block(p[0], p[1], p[2], 0, *ids.iter().next().unwrap());
        }
        let blocks: Vec<_> = cells(0, 0).map(|p| (p, world.get(p[0] >> 4, p[2] >> 4).unwrap().read().block(p[0], p[1], p[2]))).collect();
        let brute = |center: [i32; 3], ids: &HashSet<u32>, r: u32, count: usize| {
            let d = |p: [i32; 3]| p.iter().zip(center).map(|(a, c)| i64::from(a - c).pow(2)).sum::<i64>();
            let mut hits: Vec<_> =
                blocks.iter().filter(|(p, id)| d(*p) <= i64::from(r).pow(2) && ids.contains(id)).map(|&(p, _)| (d(p), p)).collect();
            hits.sort();
            hits.into_iter().take(count).map(|(_, p)| p).collect::<Vec<_>>()
        };
        for _ in 0..200 {
            let center = [next(64) - 24, next(48) + 56, next(64) - 24];
            let (r, count) = (next(20) as u32, next(30) as usize + 1);
            let ids = if next(2) == 0 { &stone } else { &dirt };
            assert_eq!(find_blocks(&world, center, ids, r, count), brute(center, ids, r, count), "{center:?} r {r} count {count}");
        }
    }
}
