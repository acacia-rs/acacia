//! How light steps from block to block: the renderer's Bedrock-like rules, or Java's
//! (`LightEngine`: a step costs the entered block's dampening but at least 1, and light does not
//! cross a face that the two blocks' face occlusion shapes cover between them).

pub use crate::blocks::FaceShapes;

/// The Java direction of each of [`super::propagate`]'s steps, and its opposite.
const JAVA_DIR: [(usize, usize); 6] = [(5, 4), (4, 5), (1, 0), (0, 1), (3, 2), (2, 3)];

#[derive(Clone, Copy)]
pub enum Rules<'a> {
    /// A step loses 1 + the entered block's filter; sky light at 15 going down loses only the filter.
    Bedrock,
    /// Java's, with the shapes cell shape `n` (from 1) indexes.
    Java(&'a [FaceShapes]),
}

impl Rules<'_> {
    /// Level after entering a block with `filter` (Java's dampening).
    #[inline]
    pub fn step(self, level: u8, filter: u8, sky_down: bool) -> u8 {
        match self {
            Rules::Bedrock if sky_down && level == 15 => 15u8.saturating_sub(filter),
            Rules::Bedrock => level.saturating_sub(1 + filter),
            Rules::Java(_) => level.saturating_sub(filter.max(1)),
        }
    }

    /// `LightEngine.shapeOccludes`: whether the face between a block of shape `from` and its
    /// neighbour of shape `to` along step `dir` is closed. Shape 0 is `isEmptyShape`.
    #[inline]
    pub fn occludes(self, from: u8, to: u8, dir: usize) -> bool {
        let Rules::Java(shapes) = self else { return false };
        if from == 0 && to == 0 {
            return false;
        }
        let (out, back) = JAVA_DIR[dir];
        let face = |shape: u8, d: usize| if shape == 0 { [0; 4] } else { shapes[usize::from(shape) - 1][d] };
        let (a, b) = (face(from, out), face(to, back));
        (0..4).all(|i| a[i] | b[i] == u64::MAX)
    }

    /// `ChunkSkyLightSources.isEdgeOccluded`: whether sky light stops coming straight down from a
    /// block of shape `above` into one with `filter` and `shape`.
    #[inline]
    pub fn edge_occluded(self, above: u8, filter: u8, shape: u8) -> bool {
        filter != 0 || self.occludes(above, shape, DOWN_STEP)
    }

    pub fn is_java(self) -> bool {
        matches!(self, Rules::Java(_))
    }
}

/// [`super::propagate`]'s step down.
pub const DOWN_STEP: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: [u64; 4] = [u64::MAX; 4];
    /// The lower half of a side face, or the whole bottom face, of a bottom slab.
    const LOWER: [u64; 4] = [u64::MAX, u64::MAX, 0, 0];
    const UPPER: [u64; 4] = [0, 0, u64::MAX, u64::MAX];

    #[test]
    fn slabs_close_a_face_only_together() {
        let bottom: FaceShapes = [FULL, [0; 4], LOWER, LOWER, LOWER, LOWER];
        let top: FaceShapes = [[0; 4], FULL, UPPER, UPPER, UPPER, UPPER];
        let rules = Rules::Java(&[[[0; 4]; 6], bottom, top]);
        // East of a bottom slab, a top slab: their side faces together cover the face.
        assert!(rules.occludes(2, 3, 0));
        assert!(!rules.occludes(2, 2, 0));
        assert!(!rules.occludes(2, 0, 0));
        // Down from a top slab's empty bottom into a bottom slab's empty top.
        assert!(!rules.occludes(3, 2, DOWN_STEP));
        // Sky light comes down into a bottom slab (open top), not into a top slab.
        assert!(!rules.edge_occluded(0, 0, 2));
        assert!(rules.edge_occluded(0, 0, 3));
        assert_eq!((rules.step(15, 0, true), rules.step(15, 1, false), rules.step(9, 3, false)), (14, 14, 6));
    }
}
