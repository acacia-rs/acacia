use acacia_registry::BlockState;

pub trait Blocks {
    /// The Java state id at a position; `None` when the chunk isn't loaded.
    fn state(&self, x: i32, y: i32, z: i32) -> Option<u32>;

    fn min_y(&self) -> i32;

    /// The states of column (x, z) from `y` up, one per slot of `out`. Override it to find the chunk once.
    fn column(&self, x: i32, z: i32, y: i32, out: &mut [Option<u32>]) {
        for (dy, slot) in (0..).zip(out.iter_mut()) {
            *slot = self.state(x, y + dy, z);
        }
    }

    fn block_state(&self, pos: [i32; 3]) -> BlockState {
        self.state(pos[0], pos[1], pos[2])
            .and_then(BlockState::from_id)
            .unwrap_or(BlockState::AIR)
    }

    fn has_chunk(&self, pos: [i32; 3]) -> bool {
        self.state(pos[0], pos[1], pos[2]).is_some()
    }
}
