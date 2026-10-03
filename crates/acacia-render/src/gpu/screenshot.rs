//! Frame capture to PNG, for checking output without looking at the window.

use std::path::Path;

/// Copies `texture` (a presented surface frame created with `COPY_SRC`) into a PNG.
pub fn save(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, path: &Path) -> Result<(), String> {
    let (w, h) = (texture.width(), texture.height());
    let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).map_err(|e| e.to_string())?;
    let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let bgra = matches!(texture.format(), wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for px in data[(y * row) as usize..][..(w * 4) as usize].as_chunks::<4>().0 {
            rgba.extend_from_slice(&if bgra { [px[2], px[1], px[0], 255] } else { [px[0], px[1], px[2], 255] });
        }
    }
    image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8).map_err(|e| e.to_string())
}
