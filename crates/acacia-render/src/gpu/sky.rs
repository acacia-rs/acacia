//! Sun, moon and stars: quads on a sphere around the camera, turned with the time of day and
//! added onto the sky colour before anything else draws (`sky.wgsl`, after Java's `SkyRenderer`).

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::entity_textures;
use super::pipeline::DEPTH_FORMAT;
use crate::sky::{Sky, SkyTextures};

const DISTANCE: f32 = 100.0;
const SUN_HALF: f32 = 30.0;
const MOON_HALF: f32 = 20.0;
const STARS: usize = 1500;
const KIND_SUN: u32 = 0;
const KIND_MOON: u32 = 1;
const KIND_STAR: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    /// At noon, before the sky's turn.
    position: [f32; 3],
    kind: u32,
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Celestial {
    turn: [[f32; 4]; 4],
    /// x: star brightness, y: moon phase.
    params: [f32; 4],
}

pub struct SkyPass {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
}

/// Two triangles around `centre`, spanned by `right` and `down` as the texture runs.
fn quad(out: &mut Vec<Vertex>, kind: u32, centre: Vec3, right: Vec3, down: Vec3) {
    let corner = |u: f32, v: f32| Vertex { position: (centre + right * (u * 2.0 - 1.0) + down * (v * 2.0 - 1.0)).to_array(), kind, uv: [u, v] };
    out.extend([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(u, v)| corner(u, v)));
}

/// The same stars every run: xorshift from a fixed seed.
fn stars(out: &mut Vec<Vertex>) {
    let mut state = 10842u64;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32
    };
    let mut made = 0;
    while made < STARS {
        let p = Vec3::new(random(), random(), random()) * 2.0 - 1.0;
        let (size, roll) = (0.15 + random() * 0.1, random() * TAU);
        // Points outside the unit ball would crowd the cube's corners.
        if p.length_squared() > 1.0 || p.length_squared() < 0.01 {
            continue;
        }
        let dir = p.normalize();
        let side = dir.any_orthonormal_vector();
        let (sin, cos) = roll.sin_cos();
        let right = (side * cos + dir.cross(side) * sin) * size;
        quad(out, KIND_STAR, dir * DISTANCE, right, dir.cross(right));
        made += 1;
    }
}

impl SkyPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, color: wgpu::TextureFormat, globals: &wgpu::Buffer, textures: &SkyTextures) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("sky.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let uniform_type = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let texture_type = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky"),
            entries: &[
                entry(0, uniform_type),
                entry(1, uniform_type),
                entry(2, texture_type),
                entry(3, texture_type),
                entry(4, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sky"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let additive = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Float32x2],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            // Behind everything: never written, so the terrain draws over it.
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: color,
                    blend: Some(wgpu::BlendState { color: additive, alpha: additive }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let mut vertices = Vec::with_capacity((STARS + 2) * 6);
        quad(&mut vertices, KIND_SUN, Vec3::Y * DISTANCE, Vec3::X * SUN_HALF, Vec3::Z * SUN_HALF);
        quad(&mut vertices, KIND_MOON, Vec3::NEG_Y * DISTANCE, Vec3::X * MOON_HALF, Vec3::NEG_Z * MOON_HALF);
        stars(&mut vertices);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky"),
            size: size_of::<Celestial>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view = |image: &image::RgbaImage| entity_textures::upload(device, queue, image.width(), image.height(), image);
        let (sun, moon) = (view(&textures.sun), view(&textures.moon));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("sky"), ..Default::default() });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&sun) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&moon) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        SkyPass { pipeline, uniform, bind_group, vertices: vertex_buffer, count: vertices.len() as u32 }
    }

    pub fn prepare(&self, queue: &wgpu::Queue, sky: &Sky, moon_phase: u8) {
        // The sun rises in the east (+x): the sky turns around the north-south axis.
        let turn = Mat4::from_rotation_y(-FRAC_PI_2) * Mat4::from_rotation_x(sky.turn * TAU);
        let celestial = Celestial { turn: turn.to_cols_array_2d(), params: [sky.stars, f32::from(moon_phase), sky.celestial, 0.0] };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&celestial));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}
