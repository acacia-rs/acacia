//! The block texture array on the GPU; animated layers are rewritten in place as game ticks pass.

use crate::assets::flipbook::{Animation, Atlas};
use crate::assets::image::{TEXTURE_SIZE, Texture};

pub struct BlockTextures {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    animations: Vec<Animation>,
    /// The tick the animated layers show.
    tick: u64,
}

impl BlockTextures {
    /// Uploads the layers as a 16×16 array with a full mip chain.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &Atlas) -> BlockTextures {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blocks"),
            size: wgpu::Extent3d { width: TEXTURE_SIZE, height: TEXTURE_SIZE, depth_or_array_layers: atlas.layers.len() as u32 },
            mip_level_count: TEXTURE_SIZE.ilog2() + 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, tex) in atlas.layers.iter().enumerate() {
            write_layer(queue, &texture, layer as u32, tex);
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        BlockTextures { texture, view, animations: atlas.animations.clone(), tick: 0 }
    }

    /// Shows the animated layers as of game tick `tick` (20 per second).
    pub fn animate(&mut self, queue: &wgpu::Queue, tick: u64) {
        if tick == self.tick {
            return;
        }
        self.tick = tick;
        for a in &self.animations {
            write_layer(queue, &self.texture, u32::from(a.layer), &a.at(tick));
        }
    }
}

fn write_layer(queue: &wgpu::Queue, texture: &wgpu::Texture, layer: u32, tex: &Texture) {
    let levels = std::iter::once(tex.rgba.to_vec()).chain(tex.mips());
    for (mip, data) in levels.enumerate() {
        let side = TEXTURE_SIZE >> mip;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture, mip_level: mip as u32, origin: wgpu::Origin3d { x: 0, y: 0, z: layer }, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side * 4), rows_per_image: Some(side) },
            wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        );
    }
}
