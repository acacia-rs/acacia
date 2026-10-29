//! Where a boat can float: still water at one surface level, with room above, read once per column.

use std::cell::RefCell;
use std::collections::HashMap;

use acacia_physics::{Aabb, LiquidKind, WorldView};

/// Half the boat box's width (`acacia_physics::boat`).
pub(crate) const HALF_WIDTH: f32 = 0.7;
const EPS: f32 = 1e-3;

/// Still-water columns at water block level `y`, memoized.
pub(crate) struct WaterGrid<'w, W: WorldView + ?Sized> {
    world: &'w W,
    pub y: i32,
    cells: RefCell<HashMap<(i32, i32), bool>>,
}

impl<'w, W: WorldView + ?Sized> WaterGrid<'w, W> {
    pub fn new(world: &'w W, y: i32) -> Self {
        Self { world, y, cells: RefCell::new(HashMap::new()) }
    }

    /// A source of still water with nothing above it a boat would touch; unloaded columns are not.
    pub fn floats(&self, x: i32, z: i32) -> bool {
        if let Some(&v) = self.cells.borrow().get(&(x, z)) {
            return v;
        }
        let v = self.read(x, z);
        self.cells.borrow_mut().insert((x, z), v);
        v
    }

    fn read(&self, x: i32, z: i32) -> bool {
        let (w, y) = (self.world, self.y);
        let cell = Aabb::new(x as f32, y as f32, z as f32, x as f32 + 1.0, y as f32 + 2.0, z as f32 + 1.0);
        if !w.is_area_loaded(&cell) {
            return false;
        }
        let here = w.block([x, y, z]);
        // Flowing water pushes, which the boat model leaves out (still water only, design decision 1).
        let still = here.liquid.is_some_and(|l| l.kind == LiquidKind::Water && l.depth == 8 && !l.falling);
        let above = w.block([x, y + 1, z]);
        let mut boxes = Vec::new();
        w.block_collisions([x, y, z], &mut boxes);
        w.block_collisions([x, y + 1, z], &mut boxes);
        still && boxes.is_empty() && above.liquid.is_none()
    }

    /// A boat centred at (x, z) with `pad` extra on each side touches only floating columns.
    pub fn clear(&self, x: f32, z: f32, pad: f32) -> bool {
        let h = HALF_WIDTH + pad;
        let (x0, x1) = ((x - h + EPS).floor() as i32, (x + h - EPS).floor() as i32);
        let (z0, z1) = ((z - h + EPS).floor() as i32, (z + h - EPS).floor() as i32);
        (x0..=x1).all(|cx| (z0..=z1).all(|cz| self.floats(cx, cz)))
    }

    /// Every point of the segment is [`clear`](Self::clear), sampled at most `STEP` apart.
    pub fn line_clear(&self, a: [f32; 2], b: [f32; 2], pad: f32) -> bool {
        const STEP: f32 = 0.2;
        let d = (b[0] - a[0]).hypot(b[1] - a[1]);
        let n = (d / STEP).ceil().max(1.0) as u32;
        (0..=n).all(|i| {
            let t = i as f32 / n as f32;
            self.clear(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, pad)
        })
    }
}

/// The water block level under a floating boat at wire height `y` (the box bottom is 0.375 lower).
pub(crate) fn water_level(y: f32) -> i32 {
    (y - 0.375).floor() as i32
}
