//! The menu blur ([`acacia_ui::blur`]) over the frame between the UI quads drawn before and after
//! it: the frame is copied out, then box-blurred back and forth between two textures.

use acacia_ui::blur::Blur;

const CONFIG_BYTES: u64 = 32;

pub struct MenuBlur {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The horizontal and the vertical pass's `Config`.
    configs: [wgpu::Buffer; 2],
    format: wgpu::TextureFormat,
    /// The window size, the two textures, and the bind groups reading the first horizontally and
    /// the second vertically.
    targets: Option<Targets>,
}

struct Targets {
    size: [u32; 2],
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    binds: [wgpu::BindGroup; 2],
}

impl MenuBlur {
    /// Draws into views of `format` (the UI pass's, non-sRGB: Java blurs gamma-space values).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> MenuBlur {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("menu blur"), source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()) });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("menu blur"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(1, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
                entry(2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("menu blur"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("menu blur"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // `"bilinear": true` on the chain's inputs; render targets clamp.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("menu blur"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let config = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: CONFIG_BYTES, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        MenuBlur { pipeline, layout, sampler, configs: [config("menu blur x"), config("menu blur y")], format, targets: None }
    }

    /// Sizes the textures to the window and sets the radius for this frame's blur.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, blur: Blur, size: [u32; 2]) {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(self.targets(device, size));
        }
        for (buffer, [dx, dy]) in self.configs.iter().zip(Blur::passes()) {
            let config = [dx, dy, size[0] as f32, size[1] as f32, blur.radius as f32, 0.0, 0.0, 0.0];
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&config));
        }
    }

    fn targets(&self, device: &wgpu::Device, [width, height]: [u32; 2]) -> Targets {
        let texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let textures = [texture("menu blur main"), texture("menu blur swap")];
        let views = textures.each_ref().map(|t| t.create_view(&Default::default()));
        let bind = |view, config: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("menu blur"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: config.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        let binds = [bind(&views[0], &self.configs[0]), bind(&views[1], &self.configs[1])];
        Targets { size: [width, height], textures, views, binds }
    }

    /// Blurs `frame` in place, `view` being its view in the blur's format. Needs `frame` to allow
    /// copying from: without that, or before [`MenuBlur::prepare`], it does nothing.
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, frame: &wgpu::Texture, view: &wgpu::TextureView) {
        let Some(t) = &self.targets else { return };
        if !frame.usage().contains(wgpu::TextureUsages::COPY_SRC) || [frame.width(), frame.height()] != t.size {
            return;
        }
        let extent = wgpu::Extent3d { width: t.size[0], height: t.size[1], depth_or_array_layers: 1 };
        encoder.copy_texture_to_texture(frame.as_image_copy(), t.textures[0].as_image_copy(), extent);
        // main -> swap across, swap -> main down, three times; the last lands in the frame.
        let passes = Blur::passes().len();
        for i in 0..passes {
            let (bind, out) = match i % 2 {
                0 => (&t.binds[0], &t.views[1]),
                _ if i + 1 == passes => (&t.binds[1], view),
                _ => (&t.binds[1], &t.views[0]),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("menu blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: out,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use wgpu::naga::front::wgsl;
    use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};

    #[test]
    fn the_shader_validates() {
        let source = include_str!("blur.wgsl");
        let module = wgsl::parse_str(source).unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{e:?}"));
    }
}
