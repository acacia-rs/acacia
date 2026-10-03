use crate::math::{BlockPos, Vec3, pos_vec};

/// Axis-aligned box in f32, matching Dragonfly's `cube.BBox32` semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// Overlap epsilon of `Aabb::intersects` (Dragonfly `IntersectsWith`).
pub const INTERSECT_EPSILON: f32 = 1e-5;

impl Aabb {
    /// Builds a box, swapping coordinates so that `min <= max`.
    pub fn new(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32) -> Self {
        let (x0, x1) = if x0 > x1 { (x1, x0) } else { (x0, x1) };
        let (y0, y1) = if y0 > y1 { (y1, y0) } else { (y0, y1) };
        let (z0, z1) = if z0 > z1 { (z1, z0) } else { (z0, z1) };
        Self { min: [x0, y0, z0], max: [x1, y1, z1] }
    }

    /// The full unit cube of a block cell.
    pub fn block(pos: BlockPos) -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).translate(pos_vec(pos))
    }

    pub fn grow(self, v: f32) -> Self {
        self.grow_vec([v, v, v])
    }

    pub fn grow_vec(mut self, v: Vec3) -> Self {
        for i in 0..3 {
            self.min[i] -= v[i];
            self.max[i] += v[i];
        }
        self
    }

    /// Expands towards the sign of each component (a swept volume).
    pub fn extend(mut self, v: Vec3) -> Self {
        for i in 0..3 {
            if v[i] < 0.0 {
                self.min[i] += v[i];
            } else if v[i] > 0.0 {
                self.max[i] += v[i];
            }
        }
        self
    }

    pub fn extend_down(mut self, d: f32) -> Self {
        self.min[1] -= d;
        self
    }

    pub fn extend_up(mut self, d: f32) -> Self {
        self.max[1] += d;
        self
    }

    pub fn translate(mut self, v: Vec3) -> Self {
        for i in 0..3 {
            self.min[i] += v[i];
            self.max[i] += v[i];
        }
        self
    }

    /// Strict overlap by more than `INTERSECT_EPSILON` on every axis; touching faces do not intersect.
    pub fn intersects(&self, o: &Aabb) -> bool {
        (0..3).all(|i| o.max[i] - self.min[i] > INTERSECT_EPSILON && self.max[i] - o.min[i] > INTERSECT_EPSILON)
    }

    /// True for empty, inverted or non-finite boxes (bedsim `BBHasZeroVolume`).
    pub fn is_empty(&self) -> bool {
        (0..3).any(|i| !self.min[i].is_finite() || !self.max[i].is_finite() || self.min[i] >= self.max[i])
    }
}
