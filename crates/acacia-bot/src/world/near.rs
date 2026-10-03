//! Terrain for idle bots: of the sub-chunks they already receive, only the columns within [`RADIUS`]
//! of the player are decoded and kept, so a block click can name the server's block (BDS ignores a
//! click whose `block_runtime_id` differs). Nothing extra is
//! requested; columns the player leaves are dropped and those it enters stay unknown.

use std::collections::HashSet;

/// Chebyshev radius in chunks; covers the block reach (7.2 blocks) from anywhere in the centre chunk.
pub(super) const RADIUS: i32 = 1;
/// Delivered blobs remembered for later sections with the same content (a hash-only store holds no bytes).
pub(super) const RECENT_BLOBS: usize = 64;

#[derive(Debug, Default)]
pub(super) struct Near {
    center: Option<(i32, i32)>,
    /// Sections (chunk x, section y, chunk z) decoded or reported all air. Elsewhere the view's air
    /// is a guess: the data never arrived or was a blob held from an earlier session.
    known: HashSet<(i32, i32, i32)>,
}

impl Near {
    /// Everything until the first centring, which drops what is too far.
    pub fn covers(&self, x: i32, z: i32) -> bool {
        self.center.is_none_or(|c| within(c, x, z))
    }

    /// Centres on chunk `(x, z)`; true if the centre moved.
    pub fn recenter(&mut self, x: i32, z: i32) -> bool {
        if self.center == Some((x, z)) {
            return false;
        }
        self.center = Some((x, z));
        self.known.retain(|&(kx, _, kz)| within((x, z), kx, kz));
        true
    }

    pub fn mark(&mut self, x: i32, section_y: i32, z: i32) {
        self.known.insert((x, section_y, z));
    }

    pub fn knows(&self, x: i32, section_y: i32, z: i32) -> bool {
        self.known.contains(&(x, section_y, z))
    }

    pub fn forget(&mut self) {
        self.known.clear();
    }
}

fn within((cx, cz): (i32, i32), x: i32, z: i32) -> bool {
    (x - cx).abs() <= RADIUS && (z - cz).abs() <= RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_columns_around_the_centre() {
        let mut near = Near::default();
        assert!(near.covers(50, 50), "everything until centred");
        assert!(near.recenter(0, 0) && !near.recenter(0, 0));
        assert!(near.covers(1, -1) && !near.covers(2, 0));
        near.mark(1, 4, 1);
        near.mark(-1, 4, 0);
        near.recenter(1, 0);
        assert!(near.knows(1, 4, 1) && !near.knows(-1, 4, 0), "left behind");
    }
}
