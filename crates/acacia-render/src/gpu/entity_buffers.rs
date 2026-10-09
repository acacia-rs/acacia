//! The entity pass's buffers: the models' vertices, and the per-frame instance and bone stores.

use wgpu::util::DeviceExt;

use crate::entity::Vertex;

pub const INSTANCES: &str = "entity instances";
pub const BONES: &str = "entity bones";

pub fn vertex_buffer(device: &wgpu::Device, vertices: &[Vertex]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("entity models"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

pub fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Writes `records` to `buffer`, replacing it with a larger one first when they do not fit.
/// Returns whether it was replaced.
pub fn upload<T: bytemuck::Pod>(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &mut wgpu::Buffer, label: &str, records: &[T]) -> bool {
    let bytes: &[u8] = bytemuck::cast_slice(records);
    let grown = bytes.len() as u64 > buffer.size();
    if grown {
        *buffer = storage_buffer(device, label, (bytes.len() as u64).next_power_of_two());
    }
    queue.write_buffer(buffer, 0, bytes);
    grown
}
