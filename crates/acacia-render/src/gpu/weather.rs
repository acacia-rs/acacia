//! The rain pass (`weather.wgsl`): the quads of [`crate::weather::columns`], rain then snow each
//! with its texture, blended after the translucent terrain as 26.3's `RenderPipelines.WEATHER`
//! (depth tested, not written, no culling).

use glam::DVec3;

use super::pipeline::DEPTH_FORMAT;
use crate::weather::columns::{self, WeatherLevel};
use crate::weather::Streaks;

/// The Weather Radius option at its default.
const RADIUS: i32 = 10;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    alpha: f32,
    /// `[block, sky]` light levels.
    light: [f32; 2],
}

pub struct WeatherPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// Rain's texture, then snow's.
    bind_groups: Option<[wgpu::BindGroup; 2]>,
    vertices: wgpu::Buffer,
    /// Vertices of rain, then of snow.
    counts: [u32; 2],
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32, 3 => Float32x2],
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
        let side = 2 * *columns::RADIUS.end() as usize + 1;
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weather"),
            size: (side * side * 6 * size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        WeatherPass { pipeline, layout, globals: globals.clone(), sampler, bind_groups: None, vertices, counts: [0; 2] }
    }

    pub fn set_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, streaks: &Streaks) {
        let group = |image: &image::RgbaImage| {
            let view = super::entity_textures::upload(device, queue, image.width(), image.height(), image);
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("weather"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        self.bind_groups = Some([group(&streaks.rain), group(&streaks.snow)]);
    }

    /// `rain` 0 to 1; `ticks` the level's game time, its fraction the partial tick.
    pub fn prepare(&mut self, queue: &wgpu::Queue, level: Option<&impl WeatherLevel>, camera: DVec3, rain: f32, ticks: f64) {
        self.counts = [0; 2];
        let (Some(level), true) = (level, rain > 0.0 && self.bind_groups.is_some()) else { return };
        let (game_time, partial) = (ticks.floor() as i64, ticks.fract() as f32);
        let columns = columns::extract(level, rain, game_time, partial, camera, RADIUS);
        let triangles = |q: [columns::WeatherVertex; 4]| {
            let v = q.map(|v| Vertex { position: v.pos, uv: v.uv, alpha: f32::from(v.alpha) / 255.0, light: v.light.map(f32::from) });
            [v[0], v[1], v[2], v[2], v[3], v[0]]
        };
        let out: Vec<Vertex> = columns::quads(&columns, camera).into_iter().flat_map(triangles).collect();
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&out));
        self.counts = [columns.rain.len() as u32 * 6, columns.snow.len() as u32 * 6];
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(groups) = &self.bind_groups else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        let mut first = 0;
        for (group, &count) in groups.iter().zip(&self.counts) {
            if count > 0 {
                pass.set_bind_group(0, group, &[]);
                pass.draw(first..first + count, 0..1);
            }
            first += count;
        }
    }
}
