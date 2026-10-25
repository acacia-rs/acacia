//! The cloud pass (`clouds.wgsl`): the faces of [`crate::clouds::build_mesh`], rebuilt when the
//! camera's cell, side of the layer or the cloud status change, drawn as 26.3's `CLOUDS` (culled)
//! and `FLAT_CLOUDS` (not) pipelines: translucent, depth tested and written.

use glam::DVec3;
use image::RgbaImage;

use super::pipeline::DEPTH_FORMAT;
use crate::clouds::{self, CloudLayer, CloudMap, RelativePos};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    shade: f32,
}

/// What the mesh was built for: cell, side of the layer, fancy, radius in cells.
type MeshKey = ([i32; 2], RelativePos, bool, i32);

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    key: MeshKey,
}

pub struct CloudPass {
    /// Fancy (culled), then flat.
    pipelines: [wgpu::RenderPipeline; 2],
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    map: Option<CloudMap>,
    mesh: Option<Mesh>,
    visible: bool,
}

impl CloudPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clouds"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("clouds.wgsl")).into()),
        });
        let entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("clouds"), entries: &[entry(0), entry(1)] });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clouds"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clouds"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: uniform.as_entire_binding() },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("clouds"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = |cull_mode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("clouds"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32],
                    })],
                },
                primitive: wgpu::PrimitiveState { cull_mode, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
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
            })
        };
        CloudPass { pipelines: [pipeline(Some(wgpu::Face::Back)), pipeline(None)], bind_group, uniform, map: None, mesh: None, visible: false }
    }

    pub fn set_texture(&mut self, image: &RgbaImage) {
        (self.map, self.mesh) = (Some(CloudMap::new(image)), None);
    }

    /// `layer` is `None` where there are no clouds or they are off; `ticks` the level's game
    /// time, its fraction the partial tick.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, camera: DVec3, layer: Option<CloudLayer>, ticks: f64) {
        self.visible = false;
        let (Some(layer), Some(map)) = (layer.filter(|l| l.color >> 24 > 0), &self.map) else { return };
        let frame = clouds::frame(map, layer.height, camera, ticks.floor() as i64, ticks.fract() as f32);
        let radius = clouds::radius_cells(layer.range);
        let key = (frame.cell, frame.pos, layer.fancy, radius);
        if self.mesh.as_ref().is_none_or(|m| m.key != key) {
            self.mesh = Some(build(device, queue, map, &frame, layer.fancy, radius, key));
        }
        let [a, r, g, b] = layer.color.to_be_bytes().map(|c| f32::from(c) / 255.0);
        let uniform = [r, g, b, a, frame.offset.x, frame.offset.y, frame.offset.z, layer.fog_end];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        self.visible = self.mesh.as_ref().is_some_and(|m| m.index_count > 0);
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(mesh) = self.mesh.as_ref().filter(|_| self.visible) else { return };
        pass.set_pipeline(&self.pipelines[usize::from(!mesh.key.2)]);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
        pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}

fn build(device: &wgpu::Device, queue: &wgpu::Queue, map: &CloudMap, frame: &clouds::CloudFrame, fancy: bool, radius: i32, key: MeshKey) -> Mesh {
    let faces = clouds::build_mesh(map, frame, fancy, radius);
    let mut vertices = Vec::with_capacity(faces.len() * 4);
    for face in &faces {
        let (corners, shade) = clouds::face_quad(face);
        vertices.extend(corners.map(|c| Vertex { position: c.to_array(), shade }));
    }
    // Quads as the game's sequential index buffer splits them.
    let indices: Vec<u32> = (0..faces.len() as u32).flat_map(|q| [0, 1, 2, 2, 3, 0].map(|i| q * 4 + i)).collect();
    let buffer = |label, bytes: &[u8], usage| {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: bytes.len().max(4) as u64, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        queue.write_buffer(&buffer, 0, bytes);
        buffer
    };
    Mesh {
        vertices: buffer("clouds", bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX),
        indices: buffer("clouds indices", bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX),
        index_count: indices.len() as u32,
        key,
    }
}
