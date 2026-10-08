//! The destroy-stage overlay on the block being mined (`crack.wgsl`): each face of its outline
//! boxes with the stage's texture at block scale, multiplied onto what is drawn there.

use std::path::Path;

use glam::{DVec3, Vec3};
use image::RgbaImage;

use super::outline::Outline;
use super::pipeline::DEPTH_FORMAT;

pub const STAGES: u32 = 10;
const MAX_BOXES: usize = 16;
const VERTICES_PER_BOX: usize = 36;
/// Off the faces it covers, so it never fights them; less than the outline's lift.
const INFLATE: f32 = 0.001;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
}

/// The ten stages stacked top to bottom, from a look's files: Bedrock's `textures/environment`,
/// else Java's `textures/block`. `None` when any is missing.
pub fn load_stages(files: &Path) -> Option<RgbaImage> {
    let mut strip = RgbaImage::new(16, 16 * STAGES);
    for stage in 0..STAGES {
        let file = ["textures/environment", "textures/block"].iter().map(|d| files.join(format!("{d}/destroy_stage_{stage}.png"))).find(|f| f.is_file())?;
        let image = image::open(&file).ok()?.resize_exact(16, 16, image::imageops::FilterType::Nearest).to_rgba8();
        image::imageops::replace(&mut strip, &image, 0, i64::from(stage * 16));
    }
    Some(strip)
}

pub struct CrackPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    vertices: wgpu::Buffer,
    count: u32,
}

impl CrackPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("crack"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("crack.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("crack"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("crack"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let multiply = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Dst, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("crack"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                })],
            },
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: Some(wgpu::BlendState { color: multiply, alpha: multiply }), write_mask: wgpu::ColorWrites::COLOR })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("crack"),
            size: (MAX_BOXES * VERTICES_PER_BOX * size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("crack"), ..Default::default() });
        CrackPass { pipeline, layout, globals: globals.clone(), sampler, bind_group: None, vertices, count: 0 }
    }

    /// The strip from [`load_stages`].
    pub fn set_stages(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, strip: &RgbaImage) {
        let view = super::entity_textures::upload(device, queue, strip.width(), strip.height(), strip);
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("crack"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        }));
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, outline: Option<&Outline>, camera: DVec3) {
        self.count = 0;
        let Some((outline, stage)) = outline.and_then(|o| Some((o, o.crack?))) else { return };
        let origin = (outline.block.as_dvec3() - camera).as_vec3();
        let vertices: Vec<Vertex> = outline.boxes.iter().take(MAX_BOXES).flat_map(|b| box_faces(origin, b, stage.min(STAGES as u8 - 1))).collect();
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices));
        self.count = vertices.len() as u32;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bind_group) = &self.bind_group else { return };
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}

/// Six faces wound counter-clockwise from outside, the texture at block scale on each.
fn box_faces(origin: Vec3, b: &[f32; 6], stage: u8) -> Vec<Vertex> {
    let (lo, hi) = (Vec3::new(b[0], b[1], b[2]) - INFLATE, Vec3::new(b[3], b[4], b[5]) + INFLATE);
    let v0 = f32::from(stage) / STAGES as f32;
    let uv = |u: f32, v: f32| [u.clamp(0.0, 1.0), v0 + (1.0 - v).clamp(0.0, 1.0) / STAGES as f32];
    let mut out = Vec::with_capacity(VERTICES_PER_BOX);
    // Each face: an axis, its side, and the two axes its texture runs along.
    for (axis, max, (ua, va)) in [(0, false, (2, 1)), (0, true, (2, 1)), (1, false, (0, 2)), (1, true, (0, 2)), (2, false, (0, 1)), (2, true, (0, 1))] {
        let at = |a: f32, c: f32| {
            let mut p = Vec3::ZERO;
            p[axis] = if max { hi[axis] } else { lo[axis] };
            p[ua] = a;
            p[va] = c;
            p
        };
        let corners = [at(lo[ua], lo[va]), at(hi[ua], lo[va]), at(hi[ua], hi[va]), at(lo[ua], hi[va])];
        // Outward normal sign decides the winding.
        let flip = ((corners[1] - corners[0]).cross(corners[3] - corners[0])[axis] > 0.0) != max;
        let order: [usize; 6] = if flip { [0, 2, 1, 0, 3, 2] } else { [0, 1, 2, 0, 2, 3] };
        out.extend(order.map(|i| Vertex { position: (origin + corners[i]).to_array(), uv: uv(corners[i][ua], corners[i][va]) }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_point_outwards() {
        let vertices = box_faces(Vec3::ZERO, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 3);
        assert_eq!(vertices.len(), VERTICES_PER_BOX);
        for tri in vertices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(tri[i].position));
            let centre = (a + b + c) / 3.0 - Vec3::splat(0.5);
            assert!((b - a).cross(c - a).dot(centre) > 0.0, "{tri:?}");
        }
        assert!(vertices.iter().all(|v| (0.3..=0.4).contains(&v.uv[1])), "stage 3's tile");
    }
}
