/// Read access to block runtime ids at world positions. Unloaded positions read as air.
pub trait BlockAccess {
    /// Layer 0 (the block).
    fn block(&self, x: i32, y: i32, z: i32) -> u32;
    /// Layer 1 (liquid / waterlogging).
    fn liquid(&self, x: i32, y: i32, z: i32) -> u32;
}
