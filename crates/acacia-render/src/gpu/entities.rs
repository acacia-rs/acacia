//! Draws [`EntityInstance`]s: one vertex buffer holding every model, one instance record per
//! entity, one bind group per texture (model default or player skin).

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use glam::{DVec3, Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::entity_textures;
use super::pipeline::DEPTH_FORMAT;
use crate::entity::{EntityInstance, EntityModels, ModelId, Skin, Vertex};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    body: [[f32; 4]; 4],
    head: [[f32; 4]; 4],
    /// x: block light, y: sky light (0..=15).
    light: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextureKey {
    Model(ModelId),
    /// Address of the shared [`Skin`]; the map keeps the `Arc` alive, so it stays unique.
    Skin(usize),
}

pub struct EntityPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instances: wgpu::Buffer,
    capacity: usize,
    bind_group: wgpu::BindGroup,
    models: Arc<EntityModels>,
    vertices: Option<wgpu::Buffer>,
    ranges: Vec<Range<u32>>,
    looks: HashMap<TextureKey, Look>,
    draws: Vec<(Range<u32>, TextureKey)>,
}

/// GPU side of one texture: a model's default, or a player skin with the mesh it may bring.
struct Look {
    skin: Option<Arc<Skin>>,
    texture: wgpu::BindGroup,
    own_mesh: Option<wgpu::Buffer>,
}

fn vertex_buffer(device: &wgpu::Device, vertices: &[Vertex]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("entity models"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("entity instances"),
        size: (capacity * size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl EntityPass {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("entity"),
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("globals.wgsl"), include_str!("entity.wgsl")).into()),
        });
        let buffer = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity"),
            entries: &[buffer(0, wgpu::BufferBindingType::Uniform), buffer(1, wgpu::BufferBindingType::Storage { read_only: true })],
        });
        let texture_layout = entity_textures::layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("entity"),
            bind_group_layouts: &[Some(&layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Float32x3, 3 => Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("entity"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            // Models hold single planes (wings, fins) and the placement mirrors z: no culling.
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
                targets: &[Some(color.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 64;
        let instances = instance_buffer(device, capacity);
        let bind_group = Self::bind_group(device, &layout, globals, &instances);
        EntityPass {
            pipeline,
            layout,
            texture_layout,
            sampler: entity_textures::sampler(device),
            instances,
            capacity,
            bind_group,
            models: Arc::default(),
            vertices: None,
            ranges: Vec::new(),
            looks: HashMap::new(),
            draws: Vec::new(),
        }
    }

    fn bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, instances: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("entity"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: instances.as_entire_binding() },
            ],
        })
    }

    pub fn set_models(&mut self, device: &wgpu::Device, models: Arc<EntityModels>) {
        let mut vertices: Vec<Vertex> = Vec::new();
        self.ranges = models
            .models()
            .iter()
            .map(|m| {
                let first = vertices.len() as u32;
                vertices.extend_from_slice(&m.mesh.vertices);
                first..vertices.len() as u32
            })
            .collect();
        self.vertices = (!vertices.is_empty()).then(|| vertex_buffer(device, &vertices));
        self.looks.clear();
        self.models = models;
    }

    /// Uploads this frame's instances. `light` gives the (block, sky) levels at a world position.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::Buffer,
        entities: &[EntityInstance],
        camera: DVec3,
        light: impl Fn(DVec3) -> [f32; 2],
    ) {
        self.draws.clear();
        self.looks.retain(|_, look| look.skin.as_ref().is_none_or(|s| Arc::strong_count(s) > 1));
        let mut records = Vec::with_capacity(entities.len());
        for e in entities {
            let (Some(model), Some(range)) = (self.models.models().get(e.model as usize), self.ranges.get(e.model as usize)) else { continue };
            let key = match &e.skin {
                Some(skin) => TextureKey::Skin(Arc::as_ptr(skin) as usize),
                None => TextureKey::Model(e.model),
            };
            let look = self.looks.entry(key).or_insert_with(|| {
                let view = match &e.skin {
                    Some(s) => entity_textures::upload(device, queue, s.width, s.height, &s.rgba),
                    None => entity_textures::load(device, queue, &model.textures),
                };
                let own_mesh = e.skin.as_ref().and_then(|s| s.mesh.as_ref()).map(|m| vertex_buffer(device, &m.vertices));
                Look { skin: e.skin.clone(), texture: entity_textures::bind_group(device, &self.texture_layout, &view, &self.sampler), own_mesh }
            });
            let own = e.skin.as_ref().and_then(|s| s.mesh.as_ref()).filter(|_| look.own_mesh.is_some());
            let (range, pivot) = own.map_or((range.clone(), model.mesh.head_pivot), |m| (0..m.vertices.len() as u32, m.head_pivot));
            // Model space has the entity facing -z with its right at -x: mirror z, then turn.
            let body = Mat4::from_translation((e.position - camera).as_vec3())
                * Mat4::from_rotation_y(-e.yaw.to_radians())
                * Mat4::from_scale(Vec3::new(e.scale, e.scale, -e.scale));
            let turn = Mat4::from_rotation_y((e.head_yaw - e.yaw).to_radians()) * Mat4::from_rotation_x(-e.pitch.to_radians());
            let head = body * Mat4::from_translation(pivot) * turn * Mat4::from_translation(-pivot);
            let [block, sky] = light(e.position + DVec3::Y * 0.5);
            records.push(Instance { body: body.to_cols_array_2d(), head: head.to_cols_array_2d(), light: [block, sky, 0.0, 0.0] });
            self.draws.push((range, key));
        }
        if records.len() > self.capacity {
            self.capacity = records.len().next_power_of_two();
            self.instances = instance_buffer(device, self.capacity);
            self.bind_group = Self::bind_group(device, &self.layout, globals, &self.instances);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&records));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(vertices) = self.vertices.as_ref().filter(|_| !self.draws.is_empty()) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        for (index, (range, key)) in self.draws.iter().enumerate() {
            let look = &self.looks[key];
            pass.set_bind_group(1, &look.texture, &[]);
            pass.set_vertex_buffer(0, look.own_mesh.as_ref().unwrap_or(vertices).slice(..));
            pass.draw(range.clone(), index as u32..index as u32 + 1);
        }
    }
}
