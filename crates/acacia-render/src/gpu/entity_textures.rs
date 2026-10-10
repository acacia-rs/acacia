//! Entity textures: one 2D texture and bind group each, since sizes differ per model.

use std::path::PathBuf;

use crate::assets::image::Texture;

pub fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty, count: None };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("entity texture"),
        entries: &[
            entry(0, wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            }),
            entry(1, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
        ],
    })
}

pub fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor { label: Some("entity"), ..Default::default() })
}

pub fn bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("entity texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32, rgba: &[u8]) -> wgpu::TextureView {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("entity"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
        size,
    );
    texture.create_view(&Default::default())
}

/// Leather armour's TGA: alpha 0 is a hole, 255 greyscale leather to dye, anything between trim
/// in its own colour. `dye` is the stack's, undyed leather's (0xA06540) without one.
fn dye_leather(image: &mut image::RgbaImage, dye: Option<[u8; 3]>) {
    let dye = dye.unwrap_or([0xA0, 0x65, 0x40]);
    for p in image.pixels_mut().filter(|p| p.0[3] > 0) {
        if p.0[3] == 255 {
            for (c, dye) in p.0.iter_mut().zip(dye) {
                *c = (u16::from(*c) * u16::from(dye) / 255) as u8;
            }
        }
        p.0[3] = 255;
    }
}

/// Texture layers, the later ones laid over the first where they are opaque; the missing-texture
/// checkerboard without a readable first layer. `tint_mask` keeps the first layer's alpha, which
/// then marks the texels to tint.
pub fn load(device: &wgpu::Device, queue: &wgpu::Queue, layers: &[&PathBuf], tint_mask: bool, dye: Option<[u8; 3]>) -> wgpu::TextureView {
    let open = |f: &&PathBuf| match image::open(f) {
        Ok(image) => Some((image.into_rgba8(), f.extension().is_some_and(|e| e == "tga"))),
        Err(e) => {
            tracing::warn!(file = %f.display(), %e, "entity texture");
            None
        }
    };
    let Some((mut image, mask_alpha)) = layers.first().and_then(open) else {
        return upload(device, queue, 16, 16, &Texture::missing().rgba[..]);
    };
    let armor = layers[0].components().any(|c| c.as_os_str() == "armor");
    if mask_alpha && armor {
        dye_leather(&mut image, dye);
    } else if mask_alpha && !tint_mask {
        // TGA alpha marks tinted or overlaid texels (sheep wool, horse markings), not holes.
        image.pixels_mut().for_each(|p| p.0[3] = 255);
    }
    let size = image.dimensions();
    for (over, _) in layers[1..].iter().filter_map(open).filter(|(o, _)| o.dimensions() == size) {
        for (under, over) in image.pixels_mut().zip(over.pixels()).filter(|(_, o)| o.0[3] >= 128) {
            *under = *over;
        }
    }
    upload(device, queue, image.width(), image.height(), &image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leather_takes_the_stacks_dye_and_trim_keeps_its_colour() {
        let texels = [[200, 200, 200, 255], [90, 60, 30, 1], [7, 7, 7, 0]];
        let dyed = |dye| {
            let mut image = image::RgbaImage::from_fn(3, 1, |x, _| image::Rgba(texels[x as usize]));
            dye_leather(&mut image, dye);
            image.into_raw()
        };
        assert_eq!(dyed(Some([255, 0, 127])), [200, 0, 99, 255, 90, 60, 30, 255, 7, 7, 7, 0]);
        assert_eq!(dyed(None)[..4], [125, 79, 50, 255]);
    }
}
