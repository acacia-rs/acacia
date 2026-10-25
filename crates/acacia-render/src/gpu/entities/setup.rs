//! Making an [`EntityPass`] and rebinding its buffers and glint.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::super::entity_buffers::{BONES, INSTANCES, storage_buffer};
use super::super::entity_textures;
use super::super::glint::GlintTextures;
use super::{BoneMatrix, EntityPass, Instance, pipelines};

impl EntityPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("entity"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("../globals.wgsl"), include_str!("../entity.wgsl")).into()),
        });
        let buffer = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let [glint_item, glint_armor, glint_sampler] = GlintTextures::layout_entries(3);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity"),
            entries: &[
                buffer(0, wgpu::BufferBindingType::Uniform),
                buffer(1, wgpu::BufferBindingType::Storage { read_only: true }),
                buffer(2, wgpu::BufferBindingType::Storage { read_only: true }),
                glint_item,
                glint_armor,
                glint_sampler,
            ],
        });
        let texture_layout = entity_textures::layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("entity"),
            bind_group_layouts: &[Some(&layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let instances = storage_buffer(device, INSTANCES, 64 * size_of::<Instance>() as u64);
        let bones = storage_buffer(device, BONES, 1024 * size_of::<BoneMatrix>() as u64);
        let glint = GlintTextures::new(device);
        let bind_group = Self::bind_group(device, &layout, globals, &instances, &bones, &glint);
        let over_globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity over globals"),
            size: size_of::<super::super::globals::Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let over_bind_group = Self::bind_group(device, &layout, &over_globals, &instances, &bones, &glint);
        EntityPass {
            pipelines: pipelines::new(device, color, &pipeline_layout, &shader),
            started: Instant::now(),
            layout,
            texture_layout,
            sampler: entity_textures::sampler(device),
            instances,
            bones,
            bind_group,
            over_globals,
            over_bind_group,
            glint,
            models: Arc::default(),
            vertices: None,
            ranges: Vec::new(),
            looks: HashMap::new(),
            draws: Vec::new(),
            world_draws: 0,
        }
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        instances: &wgpu::Buffer,
        bones: &wgpu::Buffer,
        glint: &GlintTextures,
    ) -> wgpu::BindGroup {
        let buffers = [globals, instances, bones];
        let buffers = (0..).zip(buffers).map(|(binding, b)| wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() });
        let entries: Vec<_> = buffers.chain(glint.entries(3)).collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("entity"), layout, entries: &entries })
    }

    pub fn set_glint(&mut self, device: &wgpu::Device, globals: &wgpu::Buffer, glint: GlintTextures) {
        self.glint = glint;
        self.rebind(device, globals);
    }

    pub(super) fn rebind(&mut self, device: &wgpu::Device, globals: &wgpu::Buffer) {
        self.bind_group = Self::bind_group(device, &self.layout, globals, &self.instances, &self.bones, &self.glint);
        self.over_bind_group = Self::bind_group(device, &self.layout, &self.over_globals, &self.instances, &self.bones, &self.glint);
    }

    pub(in crate::gpu) fn set_over_globals(&self, queue: &wgpu::Queue, globals: &super::super::globals::Globals) {
        queue.write_buffer(&self.over_globals, 0, bytemuck::bytes_of(globals));
    }
}
