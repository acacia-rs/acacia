//! The enchantment glint's textures on the GPU ([`crate::glint`]), bound by the entity pass and the
//! UI pass at three bindings in a row: the item's, the armour's, their sampler.

use super::Renderer;
use crate::glint::Images;

#[derive(Clone)]
pub struct GlintTextures {
    item: wgpu::TextureView,
    armor: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl GlintTextures {
    /// Black until a look's are set, so the glint adds nothing.
    pub fn new(device: &wgpu::Device) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glint"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        GlintTextures { item: texture(device, 1, 1).create_view(&Default::default()), armor: texture(device, 1, 1).create_view(&Default::default()), sampler }
    }

    fn with(&self, device: &wgpu::Device, queue: &wgpu::Queue, images: &Images) -> Self {
        let upload = |image: &image::RgbaImage| {
            let (width, height) = image.dimensions();
            let texture = texture(device, width, height);
            let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) };
            queue.write_texture(texture.as_image_copy(), image, layout, texture.size());
            texture.create_view(&Default::default())
        };
        GlintTextures { item: upload(&images.item), armor: upload(&images.armor), sampler: self.sampler.clone() }
    }

    pub fn layout_entries(first: u32) -> [wgpu::BindGroupLayoutEntry; 3] {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty, count: None };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        [entry(first, texture), entry(first + 1, texture), entry(first + 2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering))]
    }

    pub fn entries(&self, first: u32) -> [wgpu::BindGroupEntry<'_>; 3] {
        [
            wgpu::BindGroupEntry { binding: first, resource: wgpu::BindingResource::TextureView(&self.item) },
            wgpu::BindGroupEntry { binding: first + 1, resource: wgpu::BindingResource::TextureView(&self.armor) },
            wgpu::BindGroupEntry { binding: first + 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
        ]
    }
}

/// Not an sRGB format: the shaders add the texels as the sRGB values they are, as Java does.
fn texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glint"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

impl Renderer {
    /// The glint over enchanted items and armour from now on, from [`Images::load`].
    pub fn set_glint_textures(&mut self, images: &Images) {
        let textures = self.entities.glint.with(&self.device, &self.queue, images);
        self.ui_pass.set_glint(textures.clone());
        self.entities.set_glint(&self.device, &self.globals, textures);
    }
}
