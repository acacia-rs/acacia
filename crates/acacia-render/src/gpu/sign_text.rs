//! The sign text pass (`sign_text.wgsl`): each near sign's glyph quads from the UI atlas, put on
//! its board; depth-tested against the board just behind, not written, a glowing text's outline
//! under the text by draw order.

use acacia_ui::Quad;
use glam::{DVec3, Mat4, Vec3};

use super::pipeline::DEPTH_FORMAT;
use crate::sign_text::{OUTLINE_REACH, Placed, REACH};

/// Glowing text ignores the light around it.
const FULL_LIGHT: [f32; 2] = [15.0, 15.0];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    /// From the camera.
    position: [f32; 3],
    /// Atlas texels.
    uv: [f32; 2],
    color: [u8; 4],
    /// Block and sky light.
    light: [f32; 2],
}

pub struct SignTextPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    globals: wgpu::Buffer,
    /// For the UI atlas of this version.
    bind_group: Option<(wgpu::BindGroup, u64)>,
    vertices: wgpu::Buffer,
    count: u32,
}

impl SignTextPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sign text"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("sign_text.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sign text"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(1, texture),
                entry(2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sign text"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sign text"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4, 3 => Float32x2],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
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
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: None, write_mask: wgpu::ColorWrites::COLOR })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("sign text"), ..Default::default() });
        SignTextPass { pipeline, layout, sampler, globals: globals.clone(), bind_group: None, vertices: vertex_buffer(device, 4096), count: 0 }
    }

    /// Uploads the quads of the signs within reach of `camera`. `atlas` is the UI pass's texture
    /// and its version; `light` gives (block, sky) at a world position.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: Option<&(wgpu::TextureView, u64)>, signs: &[Placed], camera: DVec3, light: impl Fn(DVec3) -> [f32; 2]) {
        self.count = 0;
        let Some((view, version)) = atlas else { return };
        if self.bind_group.as_ref().is_none_or(|(_, built)| built != version) {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sign text"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.bind_group = Some((bind_group, *version));
        }
        let mut data = Vec::new();
        for sign in signs {
            let centre = sign.block.as_dvec3() + 0.5;
            let distance = centre.distance_squared(camera);
            if distance >= REACH * REACH {
                continue;
            }
            let (corner, lit) = ((sign.block.as_dvec3() - camera).as_vec3(), light(centre));
            for (matrix, face) in sign.faces.iter().zip([&sign.text.front, &sign.text.back]) {
                let outlined = face.outline_always || distance < OUTLINE_REACH * OUTLINE_REACH;
                let light = if face.glowing { FULL_LIGHT } else { lit };
                for quad in face.outline.iter().filter(|_| outlined).chain(&face.text) {
                    push_quad(&mut data, quad, matrix, corner, light);
                }
            }
        }
        if (data.len() * size_of::<Vertex>()) as u64 > self.vertices.size() {
            self.vertices = vertex_buffer(device, data.len().next_power_of_two());
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&data));
        self.count = data.len() as u32;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some((bind_group, _)) = self.bind_group.as_ref().filter(|_| self.count > 0) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}

/// Two triangles for `quad`, whose `matrix` takes font pixels to blocks from the sign's `corner`.
fn push_quad(out: &mut Vec<Vertex>, quad: &Quad, matrix: &Mat4, corner: Vec3, light: [f32; 2]) {
    let ([l, t, r, b], [ul, vt, ur, vb]) = (quad.rect, quad.uv);
    let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex { position: (corner + matrix.transform_point3(Vec3::new(x, y, 0.0))).to_array(), uv: [u, v], color: quad.color, light };
    out.extend([vertex(l, t, ul, vt), vertex(l, b, ul, vb), vertex(r, b, ur, vb), vertex(l, t, ul, vt), vertex(r, b, ur, vb), vertex(r, t, ur, vt)]);
}

fn vertex_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sign text"),
        size: (count * size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quad_becomes_two_triangles_on_the_board() {
        let quad = Quad { rect: [-2.0, -1.0, 2.0, 1.0], uv: [10.0, 20.0, 14.0, 22.0], color: [1, 2, 3, 255], glint: false };
        let matrix = Mat4::from_translation(Vec3::new(0.5, 0.5, 0.5)) * Mat4::from_scale(Vec3::new(0.25, -0.25, 0.25));
        let mut out = Vec::new();
        push_quad(&mut out, &quad, &matrix, Vec3::new(10.0, 0.0, 0.0), [7.0, 15.0]);
        let corners: Vec<([f32; 3], [f32; 2])> = out.iter().map(|v| (v.position, v.uv)).collect();
        assert_eq!(corners[0], ([10.0, 0.75, 0.5], [10.0, 20.0]), "the top left is up: font pixels run down");
        assert_eq!(corners[2], ([11.0, 0.25, 0.5], [14.0, 22.0]));
        assert_eq!((corners[3], corners[4]), (corners[0], corners[2]));
        assert!(out.iter().all(|v| v.color == [1, 2, 3, 255] && v.light == [7.0, 15.0]));
    }
}
