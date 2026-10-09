//! The shadow pass (`shadows.wgsl`): [`crate::shadows`] quads blended onto the ground after the
//! terrain, depth-tested without writing depth.

use glam::DVec3;

use super::Renderer;
use super::pipeline::DEPTH_FORMAT;
use crate::shadows::{self, ShadowVertex};

impl Renderer {
    /// Lays this frame's [`Renderer::shadows`] on the loaded ground.
    pub(super) fn prepare_shadows(&mut self, camera: DVec3) {
        let vertices = match &self.scene {
            Some(scene) => shadows::quads(&self.shadows, camera, shadows::full_cubes(scene.world())),
            None => Vec::new(),
        };
        self.shadow_pass.prepare(&self.device, &self.queue, &vertices);
    }
}

pub struct ShadowPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
    count: u32,
}

impl ShadowPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadows"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("shadows.wgsl")).into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadows"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadows"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("shadows"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadows"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShadowVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32],
                })],
            },
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
        ShadowPass { pipeline, bind_group, vertices: None, count: 0 }
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[ShadowVertex]) {
        self.count = vertices.len() as u32;
        if vertices.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        if self.vertices.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadows"),
                size: bytes.len().next_power_of_two() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.vertices.as_ref().expect("made above"), 0, bytes);
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(vertices) = self.vertices.as_ref().filter(|_| self.count > 0) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}
