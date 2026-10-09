//! Sun, moon and stars: quads on a sphere around the camera, turned with the time of day and
//! added onto the sky colour before anything else draws (`sky.wgsl`, after Java's `SkyRenderer`).

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::entity_textures;
use super::pipeline::DEPTH_FORMAT;
use crate::sky::{Realm, Sky, SkyTextures};

const DISTANCE: f32 = 100.0;
const SUN_HALF: f32 = 30.0;
const MOON_HALF: f32 = 20.0;
const STARS: usize = 1500;
const KIND_SUN: u32 = 0;
const KIND_MOON: u32 = 1;
const KIND_STAR: u32 = 2;
const KIND_GLOW: u32 = 3;
const KIND_DOME: u32 = 4;
const KIND_END: u32 = 5;
/// Points on the sunrise fan's rim (Java's 16), and on the sky disc's.
const GLOW_RIM: usize = 16;
/// Java's sky disc: 16 blocks overhead, 512 across each way.
const DOME_HEIGHT: f32 = 16.0;
const DOME_RADIUS: f32 = 512.0;

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
    /// x: star brightness, y: moon phase, z: sun and moon visibility, w: the glow's side.
    params: [f32; 4],
    glow: [f32; 4],
    /// rgb: the sky's colour overhead, linear.
    zenith: [f32; 4],
}

pub struct SkyPass {
    pipeline: wgpu::RenderPipeline,
    /// Draws the first vertices blended, not added: the sky disc and the sunrise fan.
    glow: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    /// Vertices of the overworld's sky, and of the End's box after them.
    count: u32,
    end_count: u32,
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
                entry(5, texture_type),
            ],
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sky"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let additive = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let pipeline = |blend| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: Some(blend), write_mask: wgpu::ColorWrites::COLOR })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // Java blends the glow over the sky, then adds the sun, moon and stars.
        let (glow, pipeline) = (pipeline(wgpu::BlendState::ALPHA_BLENDING), pipeline(wgpu::BlendState { color: additive, alpha: additive }));

        let mut vertices = Vec::with_capacity((STARS + 2) * 6 + GLOW_RIM * 6);
        for i in 0..GLOW_RIM {
            let rim = |i: usize| {
                let (sin, cos) = (i as f32 * TAU / GLOW_RIM as f32).sin_cos();
                Vertex { position: [cos * DOME_RADIUS, DOME_HEIGHT, sin * DOME_RADIUS], kind: KIND_DOME, uv: [0.0; 2] }
            };
            vertices.extend([Vertex { position: [0.0, DOME_HEIGHT, 0.0], kind: KIND_DOME, uv: [0.0; 2] }, rim(i), rim(i + 1)]);
        }
        for i in 0..GLOW_RIM {
            let rim = |i: usize| {
                let (sin, cos) = (i as f32 * TAU / GLOW_RIM as f32).sin_cos();
                Vertex { position: [cos, sin, 1.0], kind: KIND_GLOW, uv: [0.0; 2] }
            };
            vertices.extend([Vertex { position: [0.0; 3], kind: KIND_GLOW, uv: [0.0; 2] }, rim(i), rim(i + 1)]);
        }
        quad(&mut vertices, KIND_SUN, Vec3::Y * DISTANCE, Vec3::X * SUN_HALF, Vec3::Z * SUN_HALF);
        quad(&mut vertices, KIND_MOON, Vec3::NEG_Y * DISTANCE, Vec3::X * MOON_HALF, Vec3::NEG_Z * MOON_HALF);
        stars(&mut vertices);
        // The End's box: each face `DISTANCE` out, spanned by the other two axes.
        let sky_count = vertices.len() as u32;
        for (out, right, down) in [(Vec3::X, Vec3::Z, Vec3::Y), (Vec3::Y, Vec3::X, Vec3::Z), (Vec3::Z, Vec3::Y, Vec3::X)] {
            for side in [1.0, -1.0] {
                quad(&mut vertices, KIND_END, out * side * DISTANCE, right * DISTANCE, down * DISTANCE);
            }
        }
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
        let end = view(textures.end.as_ref().unwrap_or(&image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]))));
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
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&end) },
            ],
        });
        SkyPass { pipeline, glow, uniform, bind_group, vertices: vertex_buffer, count: sky_count, end_count: vertices.len() as u32 - sky_count }
    }

    pub fn prepare(&self, queue: &wgpu::Queue, sky: &Sky, moon_phase: u8) {
        // The sun rises in the east (+x): the sky turns around the north-south axis.
        let turn = Mat4::from_rotation_y(-FRAC_PI_2) * Mat4::from_rotation_x(sky.turn * TAU);
        let ([r, g, b, strength], side) = sky.glow.unwrap_or(([0.0; 4], 1.0));
        let [r, g, b] = super::globals::srgb_to_linear([r, g, b]);
        let [zr, zg, zb] = super::globals::srgb_to_linear(sky.color);
        let params = [sky.stars, f32::from(moon_phase), sky.celestial, side];
        let celestial = Celestial { turn: turn.to_cols_array_2d(), params, glow: [r, g, b, strength], zenith: [zr, zg, zb, 0.0] };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&celestial));
    }

    /// The Nether has no sky: the frame keeps its fog colour.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, realm: Realm) {
        // The sky disc, then the glow fan over it.
        let glow = (GLOW_RIM * 6) as u32;
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_pipeline(&self.glow);
        match realm {
            Realm::Overworld => {
                pass.draw(0..glow, 0..1);
                pass.set_pipeline(&self.pipeline);
                pass.draw(glow..self.count, 0..1);
            }
            Realm::End => pass.draw(self.count..self.count + self.end_count, 0..1),
            Realm::Nether => {}
        }
    }
}
