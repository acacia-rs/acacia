//! The rain pass (`weather.wgsl`): the columns of [`crate::weather`] as camera-facing quads, blended
//! after the translucent terrain without writing depth.

use glam::{DVec3, Vec3};

use super::pipeline::DEPTH_FORMAT;
use crate::weather::{self, Column};

/// Blocks per second rain falls; snow drifts down one texture height per 512 ticks (Java); Java's rain alpha.
const FALL: f32 = 10.0;
const SNOW_REPEATS_PER_SECOND: f32 = 20.0 / 512.0;
const ALPHA: f32 = 0.6;
/// Frames between rescans of the columns' ground.
const RESCAN: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    alpha: f32,
}

pub struct WeatherPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    vertices: wgpu::Buffer,
    count: u32,
    columns: Vec<Column>,
    frame: u32,
    /// [`weather::Streaks`]' column width and repeat.
    column_width: f32,
    blocks_per_repeat: f32,
}

impl WeatherPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("weather"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("weather.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("weather"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("weather"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("weather"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weather"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let side = 2 * weather::RADIUS as usize + 1;
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weather"),
            size: (side * side * 6 * size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        WeatherPass { pipeline, layout, globals: globals.clone(), sampler, bind_group: None, vertices, count: 0, columns: Vec::new(), frame: 0, column_width: 1.0, blocks_per_repeat: 1.0 }
    }

    pub fn set_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, streaks: &weather::Streaks) {
        let image = &streaks.image;
        (self.column_width, self.blocks_per_repeat) = (streaks.column_width, streaks.blocks_per_repeat);
        let view = super::entity_textures::upload(device, queue, image.width(), image.height(), image);
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weather"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        }));
    }

    /// `rain` 0 to 1; `seconds` drives the fall.
    pub fn prepare(&mut self, queue: &wgpu::Queue, world: Option<&acacia_world::World>, biomes: &crate::biome::BiomeColors, camera: DVec3, rain: f32, seconds: f32) {
        self.count = 0;
        let (Some(world), true) = (world, rain > 0.0 && self.bind_group.is_some()) else { return };
        if self.frame % RESCAN == 0 {
            self.columns = weather::columns(world, biomes, camera);
        }
        self.frame = self.frame.wrapping_add(1);
        let mut out = Vec::with_capacity(self.columns.len() * 6);
        for c in &self.columns {
            let centre = DVec3::new(f64::from(c.x) + 0.5, 0.0, f64::from(c.z) + 0.5);
            let to = Vec3::new((centre.x - camera.x) as f32, 0.0, (centre.z - camera.z) as f32).normalize_or(Vec3::Z);
            let side = Vec3::new(-to.z, 0.0, to.x) * (self.column_width / 2.0);
            let base = Vec3::new((centre.x - camera.x) as f32, 0.0, (centre.z - camera.z) as f32);
            // A different streak offset per column, so the rain does not fall in lockstep.
            let offset = ((c.x.wrapping_mul(3121) ^ c.z.wrapping_mul(45238971)) & 31) as f32 / 32.0;
            let alpha = rain * ALPHA * weather::fade(c, camera);
            let fallen = if c.snow { seconds * SNOW_REPEATS_PER_SECOND } else { seconds * FALL / self.blocks_per_repeat };
            let v = |y: i32| y as f32 / self.blocks_per_repeat + fallen + offset;
            // The left half is rain, the right half snow.
            let half = if c.snow { 0.5 } else { 0.0 };
            let corner = |s: f32, y: i32, u: f32| Vertex {
                position: (base + side * s + Vec3::Y * (y as f32 - camera.y as f32)).to_array(),
                uv: [u + half, -v(y)],
                alpha,
            };
            let (bl, br, tr, tl) = (corner(-1.0, c.bottom, 0.0), corner(1.0, c.bottom, 0.5), corner(1.0, c.top, 0.5), corner(-1.0, c.top, 0.0));
            out.extend([bl, br, tr, bl, tr, tl]);
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&out));
        self.count = out.len() as u32;
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
