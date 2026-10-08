//! The particle pass (`particles.wgsl`): one instance per particle, opaque with cut-out, depth-tested
//! and written like terrain.

use glam::{DVec3, Vec3};

use super::pipeline::DEPTH_FORMAT;
use crate::particles::Particle;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    centre: [f32; 3],
    size: f32,
    piece: [f32; 2],
    layer: u32,
    light: [f32; 2],
}

pub struct ParticlePass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    view: wgpu::Buffer,
    instances: wgpu::Buffer,
    count: u32,
}

impl ParticlePass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particles"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("particles.wgsl")).into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles"),
            entries: &[
                entry(0, uniform),
                entry(1, uniform),
                entry(
                    2,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(3, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("particles"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particles"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Uint32, 4 => Float32x2],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle view"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ParticlePass { pipeline, layout, view, instances: instance_buffer(device, 1024), count: 0 }
    }

    /// Uploads this frame's particles; `light` gives (block, sky) at a world position.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, particles: &[Particle], camera: DVec3, right: Vec3, up: Vec3, light: impl Fn(DVec3) -> [f32; 2]) {
        let data: Vec<Instance> = particles
            .iter()
            .map(|p| Instance {
                centre: (p.position - camera).as_vec3().to_array(),
                size: p.size,
                piece: [f32::from(p.piece[0]), f32::from(p.piece[1])],
                layer: u32::from(p.layer),
                light: light(p.position),
            })
            .collect();
        self.count = data.len() as u32;
        if data.is_empty() {
            return;
        }
        if (data.len() * size_of::<Instance>()) as u64 > self.instances.size() {
            self.instances = instance_buffer(device, data.len().next_power_of_two());
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        let uniform = [right.x, right.y, right.z, 0.0, up.x, up.y, up.z, 0.0];
        queue.write_buffer(&self.view, 0, bytemuck::cast_slice(&uniform));
    }

    /// `textures` is the block texture array the terrain draws with.
    pub fn draw(&self, device: &wgpu::Device, pass: &mut wgpu::RenderPass<'_>, globals: &wgpu::Buffer, textures: &wgpu::TextureView, sampler: &wgpu::Sampler) {
        if self.count == 0 {
            return;
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particles"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.view.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(textures) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

impl super::Renderer {
    /// The chips of the block broken at `pos`; `runtime` is its id in the shown world.
    pub fn break_particles(&mut self, pos: glam::IVec3, runtime: u32) {
        let Some(scene) = &self.scene else { return };
        let layer = scene.table().get(runtime).textures[0];
        self.particles.break_block(pos, layer);
    }

    /// Advances the particles to the game tick now, then uploads them.
    pub(super) fn prepare_particles(&mut self, camera: &crate::Camera) {
        let now = (self.started.elapsed().as_secs_f64() * 20.0) as u64;
        if let Some(scene) = &self.scene {
            let world = scene.world();
            let solid = |c: glam::IVec3| {
                let id = world.get(c.x >> 4, c.z >> 4).map(|chunk| chunk.read().block(c.x, c.y, c.z));
                id.and_then(|id| world.registry().get(id)).is_some_and(|s| s.is_solid())
            };
            // Catch up at most a second of ticks after a stall.
            for _ in 0..now.saturating_sub(self.particle_tick).min(20) {
                self.particles.tick(&solid);
            }
        }
        self.particle_tick = now;
        let light = self.scene.as_ref().map(|s| s.light().clone());
        let light = light.as_ref().map(|l| l.read());
        // Outside lit columns a particle is as bright as open sky.
        let light_at = |p: DVec3| {
            let c = p.floor().as_ivec3();
            let byte = light.as_ref().and_then(|l| l.light(c.x, c.y, c.z)).unwrap_or(15);
            [f32::from(byte >> 4), f32::from(byte & 15)]
        };
        let (forward, right) = (camera.forward(), camera.right());
        self.particle_pass.prepare(&self.device, &self.queue, &self.particles.list, camera.position, right, right.cross(forward), light_at);
    }
}

fn instance_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particles"),
        size: (count * size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
