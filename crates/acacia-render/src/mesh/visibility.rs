//! Which pairs of a section's faces see each other through non-occluding blocks (Java's
//! `VisibilityGraph`): 15 bits, one per face pair, faces in [`super::quad::DIRS`] order.

use super::volume::Volume;
use crate::blocks::BlockTable;

pub const ALL: u16 = (1 << 15) - 1;

/// Bit of the face pair `a`, `b` (a ≠ b).
const fn pair_bit(a: usize, b: usize) -> u16 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    // Pairs ordered (0,1), (0,2) .. (0,5), (1,2) ..: lo rows of decreasing length precede.
    1 << (lo * (11 - lo) / 2 + hi - lo - 1)
}

pub fn connected(visibility: u16, a: usize, b: usize) -> bool {
    a == b || visibility & pair_bit(a, b) != 0
}

/// Faces touched by a section-local cell, as a bitmask over [`super::quad::DIRS`].
fn faces_of(x: i32, y: i32, z: i32) -> u8 {
    let side = |v: i32, plus: u8, minus: u8| match v {
        15 => 1 << plus,
        0 => 1 << minus,
        _ => 0,
    };
    side(x, 0, 1) | side(y, 2, 3) | side(z, 4, 5)
}

pub fn section_visibility(v: &Volume, table: &BlockTable) -> u16 {
    let index = |x: i32, y: i32, z: i32| ((x << 8) | (z << 4) | y) as usize;
    let mut open = [false; 4096];
    let mut open_count = 0;
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let o = !table.get(v.block(x, y, z)).occludes;
                open[index(x, y, z)] = o;
                open_count += usize::from(o);
            }
        }
    }
    match open_count {
        0 => return 0,
        4096 => return ALL,
        _ => {}
    }
    let mut visibility = 0;
    let mut stack = Vec::new();
    for start in 0..4096 {
        if !open[start] {
            continue;
        }
        open[start] = false;
        stack.push(start);
        let mut faces = 0u8;
        while let Some(i) = stack.pop() {
            let (x, y, z) = ((i >> 8) as i32, (i & 15) as i32, ((i >> 4) & 15) as i32);
            faces |= faces_of(x, y, z);
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                if (0..16).contains(&nx) && (0..16).contains(&ny) && (0..16).contains(&nz) && open[index(nx, ny, nz)] {
                    open[index(nx, ny, nz)] = false;
                    stack.push(index(nx, ny, nz));
                }
            }
        }
        for a in 0..6 {
            for b in a + 1..6 {
                if faces & (1 << a) != 0 && faces & (1 << b) != 0 {
                    visibility |= pair_bit(a, b);
                }
            }
        }
        if visibility == ALL {
            break;
        }
    }
    visibility
}

#[cfg(test)]
#[test]
fn pair_bits_are_distinct_and_fill_all() {
    let bits: Vec<u16> = (0..6).flat_map(|a| (a + 1..6).map(move |b| pair_bit(a, b))).collect();
    assert_eq!(bits.iter().fold(0, |acc, b| acc | b), ALL);
    assert!(bits.iter().all(|b| b.count_ones() == 1) && bits.len() == 15);
    assert_eq!(pair_bit(4, 1), pair_bit(1, 4));
}
