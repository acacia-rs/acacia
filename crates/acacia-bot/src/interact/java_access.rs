//! The Java chunks as the world reader line-of-sight casts through.

use acacia_java_client::Chunks;
use acacia_world::BlockAccess;

const AIR: u32 = 0;

#[repr(transparent)]
pub(crate) struct JavaAccess(Chunks);

impl JavaAccess {
    pub(crate) fn new(chunks: &Chunks) -> &JavaAccess {
        // SAFETY: `JavaAccess` is a `repr(transparent)` wrapper of `Chunks`.
        unsafe { &*std::ptr::from_ref(chunks).cast::<JavaAccess>() }
    }
}

/// Unloaded cells read as air; waterlogging is part of the Java state, so there is no liquid layer.
impl BlockAccess for JavaAccess {
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.block([x, y, z]).unwrap_or(AIR)
    }

    fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
        AIR
    }
}
