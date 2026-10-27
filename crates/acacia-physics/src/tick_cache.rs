//! One tick's block lookups memoised in a dense box. A tick asks for the same few dozen cells many
//! times (liquid scans, inside-block effects, support), and a view's own lookup (the pathfinder's
//! hashed memo) costs far more than an index. The world does not change within a tick.

use std::cell::Cell;

use crate::aabb::Aabb;
use crate::math::{BlockPos, block_pos};
use crate::world::{BlockPhysics, WorldView};

/// Boxes bigger than this (a knockback sweep) are not cached.
const MAX_CELLS: i32 = 2048;

pub(crate) struct TickCache<'a, W: ?Sized> {
    inner: &'a W,
    min: BlockPos,
    size: [i32; 3],
    cells: Vec<Cell<Option<BlockPhysics>>>,
}

impl<'a, W: WorldView + ?Sized> TickCache<'a, W> {
    /// Caches the cells of `area`; lookups outside it go to `inner`.
    pub(crate) fn new(inner: &'a W, area: &Aabb) -> Self {
        let (min, max) = (block_pos(area.min), block_pos(area.max));
        let size = [0, 1, 2].map(|a| (max[a] - min[a] + 1).clamp(0, MAX_CELLS));
        let volume = size.iter().product::<i32>();
        let size = if volume > MAX_CELLS { [0; 3] } else { size };
        let cells = (0..size.iter().product::<i32>()).map(|_| Cell::new(None)).collect();
        Self { inner, min, size, cells }
    }

    fn index(&self, p: BlockPos) -> Option<usize> {
        let d = [0, 1, 2].map(|a| p[a].wrapping_sub(self.min[a]));
        (0..3).all(|a| (0..self.size[a]).contains(&d[a])).then(|| ((d[0] * self.size[1] + d[1]) * self.size[2] + d[2]) as usize)
    }
}

impl<W: WorldView + ?Sized> WorldView for TickCache<'_, W> {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        self.inner.block_collisions(pos, out);
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        let Some(cell) = self.index(pos).map(|i| &self.cells[i]) else { return self.inner.block(pos) };
        cell.get().unwrap_or_else(|| {
            let b = self.inner.block(pos);
            cell.set(Some(b));
            b
        })
    }

    fn collisions(&self, area: &Aabb, out: &mut Vec<Aabb>) {
        self.inner.collisions(area, out);
    }

    fn is_area_loaded(&self, area: &Aabb) -> bool {
        self.inner.is_area_loaded(area)
    }
}
