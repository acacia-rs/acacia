//! Java's underwater overlay (`screen_effect.wgsl`): the look's `textures/misc/underwater.png`,
//! tiled over the screen at 0.1 opacity, lit by the eye's light, sliding as the camera turns.

use super::pipeline::DEPTH_FORMAT;
use crate::camera::Camera;

/// `ScreenEffectRenderer.renderWater`: the vertex alpha, and degrees of turn per texture repeat.
const ALPHA: f32 = 0.1;
const DEGREES_PER_REPEAT: f32 = 64.0;

pub struct ScreenEffectPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    visible: bool,
}

impl ScreenEffectPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("screen effect"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("screen_effect.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen effect"),
            entries: &[entry(0, uniform), entry(1, uniform), entry(2, texture), entry(3, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering))],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("screen effect"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("screen effect"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
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
            label: Some("screen effect"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("screen effect"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        ScreenEffectPass { pipeline, layout, globals: globals.clone(), uniform, sampler, bind_group: None, visible: false }
    }

    pub fn set_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &image::RgbaImage) {
        let view = super::entity_textures::upload(device, queue, image.width(), image.height(), image);
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen effect"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        }));
    }

    /// `light` is the block and sky light at the eye.
    pub fn prepare(&mut self, queue: &wgpu::Queue, camera: &Camera, underwater: bool, light: [f32; 2]) {
        self.visible = underwater && self.bind_group.is_some();
        if !self.visible {
            return;
        }
        let half_height = 0.5 * (camera.fov_y / 2.0).tan();
        let scroll = [-camera.yaw.to_degrees() / DEGREES_PER_REPEAT, camera.pitch.to_degrees() / DEGREES_PER_REPEAT];
        let uniform = [half_height * camera.aspect, half_height, scroll[0], scroll[1], ALPHA, light[0], light[1], 0.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let (Some(bind_group), true) = (&self.bind_group, self.visible) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
