//! The flat cloud layer (`clouds.wgsl`): the pack's `clouds.png` at 12 blocks a texel, drifting west
//! at Java's 0.03 blocks a tick, blended after the terrain without writing depth.

use glam::DVec3;

use super::pipeline::DEPTH_FORMAT;

/// Blocks a texel covers, and how fast the layer drifts (blocks per second).
const TEXEL: f64 = 12.0;
const DRIFT: f64 = 0.03 * 20.0;
/// Half the layer's side in blocks.
const HALF_SIDE: f32 = 1024.0;

pub struct CloudPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: Option<(wgpu::BindGroup, [f64; 2])>,
    visible: bool,
}

impl CloudPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clouds"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("clouds.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clouds"),
            entries: &[
                entry(0, uniform),
                entry(1, uniform),
                entry(
                    2,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(3, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("clouds"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("clouds"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
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
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clouds"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("clouds"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        CloudPass { pipeline, layout, globals: globals.clone(), uniform, sampler, bind_group: None, visible: false }
    }

    pub fn set_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &image::RgbaImage) {
        let view = super::entity_textures::upload(device, queue, image.width(), image.height(), image);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clouds"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        self.bind_group = Some((bind_group, [f64::from(image.width()), f64::from(image.height())]));
    }

    /// `height` is the layer's world y, `None` where there are no clouds (the Nether, the End);
    /// `tint` from [`crate::sky::cloud_tint`].
    pub fn prepare(&mut self, queue: &wgpu::Queue, camera: DVec3, height: Option<f32>, tint: f32, seconds: f64) {
        self.visible = false;
        let (Some(height), Some((_, [w, h]))) = (height, &self.bind_group) else { return };
        // The camera in map texels, wrapped so f32 keeps its precision far out.
        let at = [((camera.x + seconds * DRIFT) / TEXEL).rem_euclid(*w), (camera.z / TEXEL).rem_euclid(*h)];
        let uniform = [at[0] as f32, at[1] as f32, (f64::from(height) - camera.y) as f32, HALF_SIDE, tint, tint, tint, 1.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        self.visible = true;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some((bind_group, _)) = &self.bind_group else { return };
        if !self.visible {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}
