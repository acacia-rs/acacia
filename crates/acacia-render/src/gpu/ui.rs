//! The UI pass (`ui.wgsl`): an [`acacia_ui::DrawList`] over the finished frame, one instance per
//! quad, sampling the UI atlas with nearest filtering so pixel art stays sharp at any GUI scale.

use acacia_ui::{Atlas, Quad};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [u8; 4],
}

pub struct UiPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    screen: wgpu::Buffer,
    /// With the atlas version it was built for and the atlas size.
    atlas: Option<(wgpu::BindGroup, u64, [f32; 2])>,
    instances: wgpu::Buffer,
    count: u32,
    /// A non-sRGB format: the UI blends in gamma space, and its colours are sRGB values.
    format: wgpu::TextureFormat,
}

impl UiPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ui"), source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()) });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(
                    1,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ui"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Unorm8x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::COLOR })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("ui"), ..Default::default() });
        let screen = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = instance_buffer(device, 1024);
        UiPass { pipeline, layout, sampler, screen, atlas: None, instances, count: 0, format: color }
    }

    /// Uploads the atlas when it changed and this frame's quads.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: &Atlas, quads: &[Quad], window: [u32; 2]) {
        if self.atlas.as_ref().is_none_or(|(_, version, _)| *version != atlas.version) {
            let image = atlas.image();
            let view = super::entity_textures::upload(device, queue, image.width(), image.height(), image);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ui"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.screen.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.atlas = Some((bind_group, atlas.version, [image.width() as f32, image.height() as f32]));
        }
        let Some((_, _, size)) = &self.atlas else { return };
        let screen = [window[0] as f32, window[1] as f32, size[0], size[1]];
        queue.write_buffer(&self.screen, 0, bytemuck::cast_slice(&screen));
        let data: Vec<Instance> = quads.iter().map(|q| Instance { rect: q.rect, uv: q.uv, color: q.color }).collect();
        let needed = (data.len() * size_of::<Instance>()) as u64;
        if needed > self.instances.size() {
            self.instances = instance_buffer(device, data.len().next_power_of_two());
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        self.count = data.len() as u32;
    }

    /// A pass of its own over the finished frame, through a view of `target` in the pass's format.
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture) {
        let Some((bind_group, _, _)) = &self.atlas else { return };
        if self.count == 0 {
            return;
        }
        let view = target.create_view(&wgpu::TextureViewDescriptor { format: Some(self.format), ..Default::default() });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

fn instance_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui quads"),
        size: (count * size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
