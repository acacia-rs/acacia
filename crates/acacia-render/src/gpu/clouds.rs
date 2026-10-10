//! The cloud pass (`clouds.wgsl`): [`crate::clouds`] boxes around the camera, drifting at Java's
//! 0.03 blocks a tick. Translucent like Java's: a depth-only pass first, then colour where the
//! depth matches, so only the nearest cloud face shows.

use glam::DVec3;
use image::RgbaImage;

use super::pipeline::DEPTH_FORMAT;
use crate::clouds::{self, CELL, CloudLayer, CloudMap, CloudVertex};

/// Blocks per second the clouds drift (+x).
const DRIFT: f64 = 0.03 * 20.0;
/// Cells drawn around the camera's cell.
const RADIUS: i32 = 32;

pub struct CloudPass {
    depth: wgpu::RenderPipeline,
    color: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    map: Option<CloudMap>,
    /// The mesh, its vertex count, the cell it is centred on and whether it is the fancy one.
    mesh: Option<(wgpu::Buffer, u32, ([i32; 2], bool))>,
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
        let pipeline = |write_depth: bool, compare, blend, mask| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("clouds"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<CloudVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32],
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write_depth),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: color, blend, write_mask: mask })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let depth = pipeline(true, wgpu::CompareFunction::Greater, None, wgpu::ColorWrites::empty());
        let color = pipeline(false, wgpu::CompareFunction::Equal, Some(wgpu::BlendState::ALPHA_BLENDING), wgpu::ColorWrites::COLOR);
        CloudPass { depth, color, bind_group, uniform, map: None, mesh: None, visible: false }
    }

    pub fn set_texture(&mut self, image: &RgbaImage) {
        (self.map, self.mesh) = (Some(CloudMap::new(image)), None);
    }

    /// `layer` is `None` where there are no clouds (the Nether, the End); `tint` from
    /// [`crate::sky::cloud_tint`].
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, camera: DVec3, layer: Option<CloudLayer>, tint: f32, seconds: f64) {
        self.visible = false;
        let (Some(CloudLayer { height, fancy }), Some(map)) = (layer, &self.map) else { return };
        let cells = [(camera.x + seconds * DRIFT) / f64::from(CELL), camera.z / f64::from(CELL)];
        let centre = cells.map(|c| c.floor() as i32);
        if self.mesh.as_ref().is_none_or(|(_, _, of)| *of != (centre, fancy)) {
            let build = if fancy { clouds::boxes } else { clouds::sheet };
            let vertices: Vec<CloudVertex> = build(map, centre[0], centre[1], RADIUS)
                .into_iter()
                .map(|v| CloudVertex { position: [v.position[0] - centre[0] as f32, v.position[1], v.position[2] - centre[1] as f32], ..v })
                .collect();
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("clouds"),
                size: (vertices.len().max(1) * size_of::<CloudVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&vertices));
            self.mesh = Some((buffer, vertices.len() as u32, (centre, fancy)));
        }
        let offset = [cells[0] - f64::from(centre[0]), cells[1] - f64::from(centre[1])];
        let reach = RADIUS as f32 * CELL;
        let uniform = [offset[0] as f32, offset[1] as f32, (f64::from(height) - camera.y) as f32, reach, tint, tint, tint, 1.0];
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        self.visible = true;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some((buffer, count, _)) = self.mesh.as_ref().filter(|_| self.visible) else { return };
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        for pipeline in [&self.depth, &self.color] {
            pass.set_pipeline(pipeline);
            pass.draw(0..*count, 0..1);
        }
    }
}
