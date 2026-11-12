#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    /// The box vanilla builds for an entity at `pos`; the half width stays a float.
    pub fn of_entity(pos: [f64; 3], width: f32, height: f32) -> Aabb {
        let w = (width / 2.0) as f64;
        Aabb {
            min: [pos[0] - w, pos[1], pos[2] - w],
            max: [pos[0] + w, pos[1] + height as f64, pos[2] + w],
        }
    }

    pub fn moved(self, d: [f64; 3]) -> Aabb {
        Aabb {
            min: [self.min[0] + d[0], self.min[1] + d[1], self.min[2] + d[2]],
            max: [self.max[0] + d[0], self.max[1] + d[1], self.max[2] + d[2]],
        }
    }

    pub fn expanded_towards(mut self, d: [f64; 3]) -> Aabb {
        for axis in 0..3 {
            if d[axis] < 0.0 {
                self.min[axis] += d[axis];
            } else if d[axis] > 0.0 {
                self.max[axis] += d[axis];
            }
        }
        self
    }

    pub fn deflated(self, by: f64) -> Aabb {
        Aabb {
            min: self.min.map(|v| v + by),
            max: self.max.map(|v| v - by),
        }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        (0..3).all(|a| self.min[a] < other.max[a] && self.max[a] > other.min[a])
    }

    /// `AABB.minmax`: the box around both.
    pub fn minmax(&self, other: &Aabb) -> Aabb {
        Aabb { min: [0, 1, 2].map(|a| self.min[a].min(other.min[a])), max: [0, 1, 2].map(|a| self.max[a].max(other.max[a])) }
    }

    pub(crate) fn contains(&self, p: [f64; 3]) -> bool {
        (0..3).all(|a| p[a] >= self.min[a] && p[a] < self.max[a])
    }

    /// `AABB.getCenter` (`Mth.lerp(0.5, min, max)`).
    fn center(&self) -> [f64; 3] {
        [0, 1, 2].map(|a| self.min[a] + 0.5 * (self.max[a] - self.min[a]))
    }

    /// `AABB.clip`: where the segment first enters this box.
    pub(crate) fn clip(&self, from: [f64; 3], to: [f64; 3]) -> Option<[f64; 3]> {
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let mut scale = 1.0;
        let mut hit = false;
        // Each axis as (a, b, c): the face's axis, then the other two in vanilla's argument order.
        for (a, b, c) in [(0, 1, 2), (1, 2, 0), (2, 0, 1)] {
            let face = if d[a] > 1.0E-7 {
                self.min[a]
            } else if d[a] < -1.0E-7 {
                self.max[a]
            } else {
                continue;
            };
            let s = (face - from[a]) / d[a];
            let pb = from[b] + s * d[b];
            let pc = from[c] + s * d[c];
            let inside = |p: f64, axis: usize| self.min[axis] - 1.0E-7 < p && p < self.max[axis] + 1.0E-7;
            if 0.0 < s && s < scale && inside(pb, b) && inside(pc, c) {
                scale = s;
                hit = true;
            }
        }
        hit.then(|| [0, 1, 2].map(|i| from[i] + scale * d[i]))
    }

    /// `AABB.collidedAlongVector`: this box swept by `travel` touches one of `parts`.
    pub(crate) fn collided_along_vector(&self, travel: [f64; 3], parts: impl IntoIterator<Item = Aabb>) -> bool {
        let from = self.center();
        let to = [from[0] + travel[0], from[1] + travel[1], from[2] + travel[2]];
        let half = [0, 1, 2].map(|a| (self.max[a] - self.min[a]) * 0.5 - 1.0E-7);
        parts.into_iter().any(|part| {
            let grown = Aabb { min: [0, 1, 2].map(|a| part.min[a] - half[a]), max: [0, 1, 2].map(|a| part.max[a] + half[a]) };
            grown.contains(to) || grown.contains(from) || grown.clip(from, to).is_some()
        })
    }
}
