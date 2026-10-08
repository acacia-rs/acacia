//! Java's `Particle.move` collision: the particle's box swept through the blocks' collision boxes,
//! y first, then the larger of x and z (`Entity.collideBoundingBox`).

use acacia_world::BlockState;
use glam::{DVec3, IVec3};

/// The blocks particles meet.
pub trait Blocks {
    /// `None` where nothing is loaded.
    fn block(&self, cell: IVec3) -> Option<&BlockState>;
    /// The liquid a block is logged with, if any.
    fn liquid(&self, cell: IVec3) -> Option<&BlockState>;
}

/// Java's `Shapes.EPSILON`: a box touching another within this blocks it.
const EPSILON: f64 = 1e-7;

/// How far a box from `min` to `max` gets of `delta` before blocks stop it.
pub fn sweep(blocks: &impl Blocks, min: DVec3, max: DVec3, delta: DVec3) -> DVec3 {
    if delta == DVec3::ZERO {
        return delta;
    }
    // Fences and walls reach half a block above their cell.
    let lo = (min.min(min + delta) - DVec3::new(0.0, 1.0, 0.0)).floor().as_ivec3();
    let hi = max.max(max + delta).floor().as_ivec3();
    let mut boxes = Vec::new();
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                let cell = IVec3::new(x, y, z);
                let Some(state) = blocks.block(cell) else { continue };
                let base = cell.as_dvec3();
                boxes.extend(state.boxes.iter().map(|b| (base + DVec3::from(b.min.map(f64::from)), base + DVec3::from(b.max.map(f64::from)))));
            }
        }
    }
    if boxes.is_empty() {
        return delta;
    }
    let (mut min, mut max) = (min, max);
    let mut out = DVec3::ZERO;
    let order = if delta.x.abs() < delta.z.abs() { [1, 2, 0] } else { [1, 0, 2] };
    for axis in order {
        let d = clip(&boxes, axis, min, max, delta[axis]);
        out[axis] = d;
        min[axis] += d;
        max[axis] += d;
    }
    out
}

fn clip(boxes: &[(DVec3, DVec3)], axis: usize, min: DVec3, max: DVec3, mut d: f64) -> f64 {
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    for (bmin, bmax) in boxes {
        let overlaps = |i: usize| bmin[i] < max[i] - EPSILON && bmax[i] > min[i] + EPSILON;
        if !overlaps(a) || !overlaps(b) {
            continue;
        }
        if d > 0.0 && bmin[axis] >= max[axis] - EPSILON {
            d = d.min(bmin[axis] - max[axis]);
        } else if d < 0.0 && bmax[axis] <= min[axis] + EPSILON {
            d = d.max(bmax[axis] - min[axis]);
        }
    }
    d
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use acacia_world::BlockRegistry;

    /// Stone below y 64, air above, and blocks placed on top.
    pub struct Floor {
        pub registry: &'static BlockRegistry,
        ids: [u32; 2],
        pub placed: Vec<(IVec3, u32)>,
    }

    impl Floor {
        pub fn new() -> Floor {
            let registry = BlockRegistry::vanilla();
            let ids = ["minecraft:stone", "minecraft:air"].map(|n| registry.find(n, "").expect("vanilla block"));
            Floor { registry, ids, placed: Vec::new() }
        }

        pub fn place(&mut self, cell: IVec3, name: &str, properties: &str) {
            let id = self.registry.find(name, properties).expect("vanilla state");
            self.placed.push((cell, id));
        }
    }

    impl Blocks for Floor {
        fn block(&self, cell: IVec3) -> Option<&BlockState> {
            let placed = self.placed.iter().find(|(c, _)| *c == cell).map(|(_, id)| *id);
            self.registry.get(placed.unwrap_or(if cell.y < 64 { self.ids[0] } else { self.ids[1] }))
        }

        fn liquid(&self, _: IVec3) -> Option<&BlockState> {
            None
        }
    }

    #[test]
    fn a_falling_box_lands_on_the_floor_and_slides_on() {
        let floor = Floor::new();
        let d = sweep(&floor, DVec3::new(0.4, 64.5, 0.4), DVec3::new(0.6, 64.7, 0.6), DVec3::new(0.3, -1.0, 0.0));
        assert!((d.y + 0.5).abs() < 1e-9 && (d.x - 0.3).abs() < 1e-9, "{d}");
    }
}
