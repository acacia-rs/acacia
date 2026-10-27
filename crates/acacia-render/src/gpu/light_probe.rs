//! Light for entities and particles without waiting on the light thread, whose write lock lasts a
//! column's relight (tens of ms while a join's view lights): while it is held, each cell answers
//! what it read last frame.

use std::cell::RefCell;

use glam::{DVec3, IVec3};
use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;

use crate::light::LightData;

#[derive(Default)]
pub(super) struct LightCache {
    prev: FxHashMap<IVec3, u8>,
    now: RefCell<FxHashMap<IVec3, u8>>,
}

impl LightCache {
    /// Starts a frame: cells nobody asked about last frame are forgotten.
    pub(super) fn next_frame(&mut self) {
        self.prev = std::mem::take(self.now.get_mut());
    }

    pub(super) fn reader<'a>(&'a self, data: Option<&'a RwLock<LightData>>) -> LightReader<'a> {
        LightReader { cache: self, data: data.and_then(RwLock::try_read) }
    }
}

pub(super) struct LightReader<'a> {
    cache: &'a LightCache,
    data: Option<RwLockReadGuard<'a, LightData>>,
}

impl LightReader<'_> {
    /// `[block, sky]` light at `p`; outside lit columns (or a cell first seen while the lock is held), open sky.
    pub(super) fn at(&self, p: DVec3) -> [f32; 2] {
        let c = p.floor().as_ivec3();
        let byte = match &self.data {
            Some(d) => d.light(c.x, c.y, c.z),
            None => self.cache.prev.get(&c).copied(),
        }
        .unwrap_or(15);
        self.cache.now.borrow_mut().insert(c, byte);
        [f32::from(byte >> 4), f32::from(byte & 15)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_lock_answers_last_frames_light() {
        let lock = RwLock::new(LightData::default());
        lock.write().light.fill((0, 0, 0), crate::light::data::pack(7, 3));
        let mut cache = LightCache::default();
        let p = DVec3::new(1.5, 2.5, 3.5);
        assert_eq!(cache.reader(Some(&lock)).at(p), [7.0, 3.0]);
        cache.next_frame();
        let held = lock.write();
        assert_eq!(cache.reader(Some(&lock)).at(p), [7.0, 3.0]);
        assert_eq!(cache.reader(Some(&lock)).at(DVec3::new(-5.0, 0.0, 0.0)), [0.0, 15.0]);
        drop(held);
    }
}
