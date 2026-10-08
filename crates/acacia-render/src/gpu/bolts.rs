//! The lightning pass (`bolts.wgsl`): [`crate::lightning`] bolts as translucent triangles added
//! onto the frame after the terrain, depth-tested without writing depth.

use glam::DVec3;

use super::pipeline::DEPTH_FORMAT;
use crate::lightning;

pub struct BoltPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
    count: u32,
}

impl BoltPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bolts"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("bolts.wgsl")).into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bolts"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bolts"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("bolts"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        // Java's lightning transparency: source alpha, added.
        let added = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bolts"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
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
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: Some(wgpu::BlendState { color: added, alpha: added }), write_mask: wgpu::ColorWrites::COLOR })],
            }),
            multiview_mask: None,
            cache: None,
        });
        BoltPass { pipeline, bind_group, vertices: None, count: 0 }
    }

    /// `bolts`: each bolt's seed and where it strikes.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bolts: &[(u64, DVec3)], camera: DVec3) {
        let mut out: Vec<[f32; 3]> = Vec::new();
        for &(seed, at) in bolts {
            let foot = (at - camera).as_vec3();
            out.extend(lightning::bolt(seed).into_iter().map(|v| (foot + v).to_array()));
        }
        self.count = out.len() as u32;
        if out.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&out);
        if self.vertices.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bolts"),
                size: bytes.len() as u64,
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
