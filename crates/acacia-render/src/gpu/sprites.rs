//! The sprite particle pass (`sprites.wgsl`): one instance per sprite from the look's particle
//! sheet, sorted far to near and blended premultiplied, so cut-out and translucent sprites share
//! one draw; depth-tested, not written.

use glam::{DVec3, Vec3};

use super::pipeline::DEPTH_FORMAT;
use crate::particles::{Blend, Light, Sheet, Sprite};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    centre: [f32; 3],
    size: f32,
    uv: [f32; 4],
    color: [f32; 4],
    light: [f32; 2],
    blend: u32,
}

pub struct SpritePass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    view: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    instances: wgpu::Buffer,
    count: u32,
}

impl SpritePass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprites"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("sprites.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprites"),
            entries: &[entry(0, uniform), entry(1, uniform), entry(2, texture), entry(3, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering))],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sprites"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let premultiplied = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Float32x4, 4 => Float32x2, 5 => Uint32],
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: color,
                    blend: Some(wgpu::BlendState { color: premultiplied, alpha: premultiplied }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite view"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = super::entity_textures::sampler(device);
        SpritePass { pipeline, layout, globals: globals.clone(), view, sampler, bind_group: None, instances: instance_buffer(device, 1024), count: 0 }
    }

    pub fn set_sheet(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sheet: &Sheet) {
        let texture = super::entity_textures::upload(device, queue, sheet.image.width(), sheet.image.height(), &sheet.image);
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprites"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.view.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&texture) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        }));
    }

    /// Uploads this frame's sprites `partial` of a tick past their last tick; `light` gives
    /// (block, sky) at a world position.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sheet: &Sheet, sprites: &[Sprite], partial: f64, camera: DVec3, right: Vec3, up: Vec3, light: impl Fn(DVec3) -> [f32; 2]) {
        self.count = 0;
        if self.bind_group.is_none() {
            return;
        }
        let mut data: Vec<(f32, Instance)> = sprites
            .iter()
            .filter(|s| s.visible())
            .filter_map(|s| {
                let frames = sheet.frames(s.set);
                let uv = *frames.get(s.frame_index(frames.len()))?;
                let at = s.previous.lerp(s.position, partial);
                let [block, sky] = light(at);
                let light = match s.light {
                    Light::World => [block, sky],
                    Light::Full => [15.0, 15.0],
                    Light::Ramp => [(block + s.life() * 15.0).min(15.0), sky],
                    Light::Block => [15.0, sky],
                };
                let blend = match s.blend {
                    Blend::Cutout => 0,
                    Blend::Alpha => 1,
                };
                let centre = (at - camera).as_vec3();
                Some((centre.length_squared(), Instance { centre: centre.to_array(), size: s.quad_size(), uv, color: s.tint(), light, blend }))
            })
            .collect();
        if data.is_empty() {
            return;
        }
        data.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
        let data: Vec<Instance> = data.into_iter().map(|(_, i)| i).collect();
        if (data.len() * size_of::<Instance>()) as u64 > self.instances.size() {
            self.instances = instance_buffer(device, data.len().next_power_of_two());
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        let uniform = [right.x, right.y, right.z, 0.0, up.x, up.y, up.z, 0.0];
        queue.write_buffer(&self.view, 0, bytemuck::cast_slice(&uniform));
        self.count = data.len() as u32;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bind_group) = &self.bind_group else { return };
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

fn instance_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sprites"),
        size: (count * size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
