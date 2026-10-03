//! GPU storage for section meshes: one quad buffer shared by all sections (first-fit free list)
//! and a buffer of section origins indexed by slot (the draw's instance index).

use glam::{IVec3, Vec3};
use rustc_hash::FxHashMap;

use crate::camera::Frustum;
use crate::mesh::{Quad, SectionMesh};
use crate::workers::SectionKey;

const QUAD_BYTES: u64 = size_of::<Quad>() as u64;
const ORIGIN_BYTES: u64 = 16;
const INITIAL_QUADS: u32 = 1 << 20;
const INITIAL_SLOTS: u32 = 4096;

pub struct Store {
    pub quads: wgpu::Buffer,
    pub origins: wgpu::Buffer,
    quad_capacity: u32,
    max_quads: u32,
    free: FreeList,
    slot_capacity: u32,
    free_slots: Vec<u32>,
    next_slot: u32,
    entries: FxHashMap<SectionKey, Entry>,
    used_quads: u64,
    /// Set when a buffer was replaced; the bind group must be rebuilt.
    pub replaced: bool,
}

struct Entry {
    slot: u32,
    offset: u32,
    solid: u32,
    translucent: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Draw {
    pub slot: u32,
    pub first: u32,
    pub count: u32,
}

impl Store {
    pub fn new(device: &wgpu::Device) -> Self {
        let max_quads = (device.limits().max_storage_buffer_binding_size / QUAD_BYTES).min(u32::MAX as u64) as u32;
        let quad_capacity = INITIAL_QUADS.min(max_quads);
        Store {
            quads: buffer(device, "quads", quad_capacity as u64 * QUAD_BYTES),
            origins: buffer(device, "section origins", INITIAL_SLOTS as u64 * ORIGIN_BYTES),
            quad_capacity,
            max_quads,
            free: FreeList { ranges: vec![(0, quad_capacity)] },
            slot_capacity: INITIAL_SLOTS,
            free_slots: Vec::new(),
            next_slot: 0,
            entries: FxHashMap::default(),
            used_quads: 0,
            replaced: false,
        }
    }

    pub fn sections(&self) -> usize {
        self.entries.len()
    }

    pub fn used_quads(&self) -> u64 {
        self.used_quads
    }

    pub fn gpu_bytes(&self) -> u64 {
        self.quad_capacity as u64 * QUAD_BYTES + self.slot_capacity as u64 * ORIGIN_BYTES
    }

    pub fn clear(&mut self) {
        let keys: Vec<_> = self.entries.keys().copied().collect();
        keys.into_iter().for_each(|k| self.remove(k));
    }

    pub fn remove(&mut self, key: SectionKey) {
        if let Some(e) = self.entries.remove(&key) {
            self.free.release(e.offset, e.solid + e.translucent);
            self.free_slots.push(e.slot);
            self.used_quads -= u64::from(e.solid + e.translucent);
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, key: SectionKey, mesh: SectionMesh) {
        self.remove(key);
        if mesh.is_empty() {
            return;
        }
        let len = (mesh.solid.len() + mesh.translucent.len()) as u32;
        let Some(offset) = self.alloc(device, queue, len) else {
            tracing::warn!(len, "quad buffer full; section dropped");
            return;
        };
        let slot = self.slot(device, queue);
        let mut data = mesh.solid;
        data.extend_from_slice(&mesh.translucent);
        queue.write_buffer(&self.quads, offset as u64 * QUAD_BYTES, bytemuck::cast_slice(&data));
        let origin = [key.0 * 16, key.1 * 16, key.2 * 16, 0];
        queue.write_buffer(&self.origins, slot as u64 * ORIGIN_BYTES, bytemuck::cast_slice(&origin));
        self.used_quads += u64::from(len);
        self.entries.insert(key, Entry { slot, offset, solid: len - mesh.translucent.len() as u32, translucent: mesh.translucent.len() as u32 });
    }

    /// Visible sections: solid front to back (early depth rejection), translucent back to front.
    pub fn draws(&self, frustum: &Frustum, cam_block: IVec3, cam_frac: Vec3) -> (Vec<Draw>, Vec<Draw>) {
        let mut visible: Vec<(i32, &Entry)> = self
            .entries
            .iter()
            .filter_map(|(&(x, y, z), e)| {
                let min = (IVec3::new(x, y, z) * 16 - cam_block).as_vec3() - cam_frac;
                frustum.intersects_box(min, min + Vec3::splat(16.0)).then(|| {
                    let d = IVec3::new(x, y, z) * 16 + 8 - cam_block;
                    (d.length_squared(), e)
                })
            })
            .collect();
        visible.sort_unstable_by_key(|(d, _)| *d);
        let solid = visible.iter().filter(|(_, e)| e.solid > 0).map(|(_, e)| Draw { slot: e.slot, first: e.offset, count: e.solid });
        let translucent =
            visible.iter().rev().filter(|(_, e)| e.translucent > 0).map(|(_, e)| Draw { slot: e.slot, first: e.offset + e.solid, count: e.translucent });
        (solid.collect(), translucent.collect())
    }

    fn alloc(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, len: u32) -> Option<u32> {
        if let Some(offset) = self.free.alloc(len) {
            return Some(offset);
        }
        let old = self.quad_capacity;
        let new = old.saturating_mul(2).max(old + len).min(self.max_quads);
        if new <= old {
            return None;
        }
        self.quads = grow(device, queue, &self.quads, "quads", old as u64 * QUAD_BYTES, new as u64 * QUAD_BYTES);
        self.free.release(old, new - old);
        self.quad_capacity = new;
        self.replaced = true;
        self.free.alloc(len)
    }

    fn slot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> u32 {
        if let Some(s) = self.free_slots.pop() {
            return s;
        }
        if self.next_slot == self.slot_capacity {
            let new = self.slot_capacity * 2;
            let (old_bytes, new_bytes) = (self.slot_capacity as u64 * ORIGIN_BYTES, new as u64 * ORIGIN_BYTES);
            self.origins = grow(device, queue, &self.origins, "section origins", old_bytes, new_bytes);
            self.slot_capacity = new;
            self.replaced = true;
        }
        self.next_slot += 1;
        self.next_slot - 1
    }
}

fn buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// Copies into a larger buffer. Submitted at once: writes already queued for the old buffer run
/// before the copy, and later writes target the new one.
fn grow(device: &wgpu::Device, queue: &wgpu::Queue, old: &wgpu::Buffer, label: &str, old_size: u64, new_size: u64) -> wgpu::Buffer {
    let new = buffer(device, label, new_size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("grow") });
    encoder.copy_buffer_to_buffer(old, 0, &new, 0, old_size);
    queue.submit([encoder.finish()]);
    new
}

/// Free ranges `(offset, len)` sorted by offset, merged when adjacent.
struct FreeList {
    ranges: Vec<(u32, u32)>,
}

impl FreeList {
    fn alloc(&mut self, len: u32) -> Option<u32> {
        let i = self.ranges.iter().position(|&(_, l)| l >= len)?;
        let (offset, l) = self.ranges[i];
        if l == len {
            self.ranges.remove(i);
        } else {
            self.ranges[i] = (offset + len, l - len);
        }
        Some(offset)
    }

    fn release(&mut self, offset: u32, len: u32) {
        if len == 0 {
            return;
        }
        let i = self.ranges.partition_point(|&(o, _)| o < offset);
        self.ranges.insert(i, (offset, len));
        if i + 1 < self.ranges.len() && self.ranges[i].0 + self.ranges[i].1 == self.ranges[i + 1].0 {
            self.ranges[i].1 += self.ranges.remove(i + 1).1;
        }
        if i > 0 && self.ranges[i - 1].0 + self.ranges[i - 1].1 == self.ranges[i].0 {
            self.ranges[i - 1].1 += self.ranges.remove(i).1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FreeList;

    #[test]
    fn free_list_reuses_and_merges() {
        let mut f = FreeList { ranges: vec![(0, 100)] };
        let (a, b, c) = (f.alloc(10).unwrap(), f.alloc(20).unwrap(), f.alloc(30).unwrap());
        assert_eq!((a, b, c), (0, 10, 30));
        f.release(b, 20);
        assert_eq!(f.alloc(15), Some(10));
        f.release(10, 15);
        f.release(a, 10);
        f.release(c, 30);
        assert_eq!(f.ranges, [(0, 100)]);
        assert_eq!(f.alloc(101), None);
    }
}
