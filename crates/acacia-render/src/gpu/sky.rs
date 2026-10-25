//! Java's `SkyRenderer`: the sky disc, sunrise fan, sun, moon, stars and dark disc, or the End's
//! box, drawn before anything else (`sky.wgsl`). Every vertex and frame comes from
//! [`crate::sky::geometry`]; the shader only places and colours them.

use glam::Mat4;
use wgpu::util::DeviceExt;

use super::entity_textures;
use super::pipeline::DEPTH_FORMAT;
use crate::sky::geometry::{self, DARK_DISC_Y, SKY_DISC_Y, SUN_SIZE};
use crate::sky::{Attr, Realm, Sky, SkyTextures};

const KIND_SUN: u32 = 0;
const KIND_MOON: u32 = 1;
const KIND_STAR: u32 = 2;
const KIND_GLOW: u32 = 3;
const KIND_DOME: u32 = 4;
const KIND_END: u32 = 5;
const KIND_DARK: u32 = 6;
/// `getHorizonHeight` outside a flat world, which the renderer cannot tell apart.
const HORIZON: f64 = 63.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    /// Model position, before its kind's frame.
    position: [f32; 3],
    kind: u32,
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Celestial {
    /// The sun's, moon's, stars', sunrise fan's and dark disc's frames.
    frames: [[[f32; 4]; 4]; 5],
    /// x: star brightness, y: moon phase, z: 1 - rain, w: `FogData.skyEnd`.
    params: [f32; 4],
    /// The sunrise colour, linear rgb and alpha.
    glow: [f32; 4],
    /// rgb: the sky colour, linear; w: 1 when the dark disc shows.
    zenith: [f32; 4],
}

pub struct SkyPass {
    /// Blends: the sky disc, sunrise fan and dark disc.
    blended: wgpu::RenderPipeline,
    /// Adds: the sun, moon and stars (Java's `OVERLAY`).
    added: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    /// Vertex ranges: disc and fan, sun to stars, dark disc, the End's box.
    ranges: [std::ops::Range<u32>; 4],
}

fn fan(out: &mut Vec<Vertex>, kind: u32, points: &[[f32; 3]]) {
    let at = |p: [f32; 3], u: f32| Vertex { position: p, kind, uv: [u, 0.0] };
    for pair in points[1..].windows(2) {
        out.extend([at(points[0], 1.0), at(pair[0], 0.0), at(pair[1], 0.0)]);
    }
}

fn quad(out: &mut Vec<Vertex>, kind: u32, corners: [([f32; 3], [f32; 2]); 4]) {
    let at = |(position, uv): ([f32; 3], [f32; 2])| Vertex { position, kind, uv };
    out.extend([0, 1, 2, 0, 2, 3].map(|i| at(corners[i])));
}

fn vertices() -> (Vec<Vertex>, [std::ops::Range<u32>; 4]) {
    let mut out = Vec::new();
    let mut ends = [0u32; 4];
    fan(&mut out, KIND_DOME, &geometry::sky_disc(SKY_DISC_Y));
    fan(&mut out, KIND_GLOW, &geometry::sunrise_fan_model());
    ends[0] = out.len() as u32;
    quad(&mut out, KIND_SUN, geometry::body(Mat4::IDENTITY, SUN_SIZE));
    quad(&mut out, KIND_MOON, geometry::moon(Mat4::IDENTITY));
    for star in geometry::stars() {
        quad(&mut out, KIND_STAR, star.map(|p| (p, [0.0; 2])));
    }
    ends[1] = out.len() as u32;
    fan(&mut out, KIND_DARK, &geometry::sky_disc(DARK_DISC_Y));
    ends[2] = out.len() as u32;
    for face in geometry::end_sky() {
        quad(&mut out, KIND_END, face);
    }
    ends[3] = out.len() as u32;
    (out, [0..ends[0], ends[0]..ends[1], ends[1]..ends[2], ends[2]..ends[3]])
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
        let (blended, added) = (pipeline(wgpu::BlendState::ALPHA_BLENDING), pipeline(wgpu::BlendState { color: additive, alpha: additive }));
        let (vertices, ranges) = vertices();
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
        SkyPass { blended, added, uniform, bind_group, vertices: vertex_buffer, ranges }
    }

    /// `sky_end` is `FogData.skyEnd`; `eye_y` and `underwater` decide the dark disc.
    pub fn prepare(&self, queue: &wgpu::Queue, sky: &Sky, sky_end: f32, eye_y: f64, underwater: bool) {
        let dark_disc = geometry::dark_disc_shown(eye_y, HORIZON, underwater);
        let a = &sky.attributes;
        let sun = a.number(Attr::SunAngle);
        let [r, g, b, alpha] = a.get(Attr::SunriseSunsetColor);
        let frames = [
            geometry::celestial_frame(sun),
            geometry::celestial_frame(a.number(Attr::MoonAngle)),
            geometry::celestial_frame(a.number(Attr::StarAngle)),
            geometry::sunrise_frame(sun, alpha),
            geometry::dark_disc_frame(),
        ]
        .map(|m| m.to_cols_array_2d());
        let [r, g, b] = super::globals::srgb_to_linear([r, g, b]);
        let [zr, zg, zb] = super::globals::srgb_to_linear(sky.color());
        let celestial = Celestial {
            frames,
            params: [a.number(Attr::StarBrightness), f32::from(a.moon_phase()), sky.celestial, sky_end],
            glow: [r, g, b, alpha],
            zenith: [zr, zg, zb, f32::from(u8::from(dark_disc))],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&celestial));
    }

    /// The Nether has no sky: the frame keeps its fog colour.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, realm: Realm) {
        let [blended, added, dark, end] = self.ranges.clone();
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_pipeline(&self.blended);
        match realm {
            Realm::Overworld => {
                pass.draw(blended, 0..1);
                pass.set_pipeline(&self.added);
                pass.draw(added, 0..1);
                pass.set_pipeline(&self.blended);
                pass.draw(dark, 0..1);
            }
            Realm::End => pass.draw(end, 0..1),
            Realm::Nether => {}
        }
    }
}
