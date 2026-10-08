//! The targeted block's outline (`outline.wgsl`): the edges of its boxes, depth-tested against the
//! terrain without writing depth.

use glam::{DVec3, IVec3, Vec3};

use super::pipeline::DEPTH_FORMAT;

/// Boxes past this many are not drawn (the most a vanilla shape has is far below it).
const MAX_BOXES: usize = 32;
const VERTICES_PER_BOX: usize = 24;
/// Java grows the outline by this so it does not fight the faces under it.
const INFLATE: f32 = 0.002;

/// What the crosshair targets: a block and its outline boxes, in blocks relative to its corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub block: IVec3,
    /// `[min x, min y, min z, max x, max y, max z]`.
    pub boxes: Vec<[f32; 6]>,
    /// Destroy stage 0 to 9 while it is being mined.
    pub crack: Option<u8>,
}

pub struct OutlinePass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
}

impl OutlinePass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("outline"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("outline.wgsl")).into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("outline"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("outline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<[f32; 3]>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                // Reverse-Z: nearer is greater.
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("outline"),
            size: (MAX_BOXES * VERTICES_PER_BOX * size_of::<[f32; 3]>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("outline"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        OutlinePass { pipeline, bind_group, vertices, count: 0 }
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, outline: Option<&Outline>, camera: DVec3) {
        let Some(outline) = outline else {
            self.count = 0;
            return;
        };
        let origin = (outline.block.as_dvec3() - camera).as_vec3();
        let lines: Vec<[f32; 3]> = outline.boxes.iter().take(MAX_BOXES).flat_map(|b| box_edges(origin, b)).collect();
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&lines));
        self.count = lines.len() as u32;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}

/// The 12 edges of a box as 24 line ends.
fn box_edges(origin: Vec3, b: &[f32; 6]) -> [[f32; 3]; VERTICES_PER_BOX] {
    let min = origin + Vec3::new(b[0], b[1], b[2]) - INFLATE;
    let max = origin + Vec3::new(b[3], b[4], b[5]) + INFLATE;
    let corner = |i: usize| Vec3::new(if i & 1 == 0 { min.x } else { max.x }, if i & 2 == 0 { min.y } else { max.y }, if i & 4 == 0 { min.z } else { max.z }).to_array();
    // Corner pairs differing in one bit.
    const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    let mut out = [[0.0; 3]; VERTICES_PER_BOX];
    for (i, (a, b)) in EDGES.iter().enumerate() {
        out[i * 2] = corner(*a);
        out[i * 2 + 1] = corner(*b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_follow_the_box() {
        let lines = box_edges(Vec3::ZERO, &[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]);
        for pair in lines.chunks(2) {
            let differ = (0..3).filter(|&i| (pair[0][i] - pair[1][i]).abs() > 0.1).count();
            assert_eq!(differ, 1, "an edge runs along one axis: {pair:?}");
        }
        assert!(lines.iter().all(|p| p[1] <= 0.5 + INFLATE + 1e-6), "slab height");
    }
}
