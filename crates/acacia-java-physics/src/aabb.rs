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
}
