//! Draws [`EntityInstance`]s: one vertex buffer holding every model, one instance record per
//! layer of an entity, one bind group per texture (a layer's composed textures or a player skin).

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use glam::{DVec3, Mat4, Vec3};

use super::entity_buffers::{BONES, INSTANCES, storage_buffer, upload, vertex_buffer};
use super::entity_textures;
use super::glint::GlintTextures;
use super::pipeline::DEPTH_FORMAT;
use crate::entity::{EntityInstance, EntityModels, Skin, TextureId, Vertex};
use crate::glint::Glint;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    /// x: block light, y: sky light (0..=15), z: 1 for the hurt overlay, w: the glint texture
    /// (0 none, 1 the item's, 2 the armour's).
    light: [f32; 4],
    /// rgb: the layer's tint, a: 1 when the texture's alpha is a tint mask and not a cutout.
    tint: [f32; 4],
    hidden: [u32; 4],
    /// x: index of the layer's first matrix in the bone buffer.
    bones: [u32; 4],
    /// xy: the glint's scale over the UVs, zw: how far it has slid ([`crate::glint`]).
    glint: [f32; 4],
}

/// Model space to camera-relative world space, one per bone of each instance.
type BoneMatrix = [[f32; 4]; 4];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextureKey {
    /// A layer's textures, and whether they compose as a tint mask.
    Layers([TextureId; 3], bool),
    /// Address of the shared [`Skin`]; the map keeps the `Arc` alive, so it stays unique.
    Skin(usize),
}

pub struct EntityPass {
    pipeline: wgpu::RenderPipeline,
    /// For [`Layer::blend`] layers.
    blend_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instances: wgpu::Buffer,
    bones: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pub glint: GlintTextures,
    models: Arc<EntityModels>,
    vertices: Option<wgpu::Buffer>,
    ranges: Vec<Range<u32>>,
    looks: HashMap<TextureKey, Look>,
    /// Vertices, texture and whether the layer is blended, per instance record.
    draws: Vec<(Range<u32>, TextureKey, bool)>,
    /// How many of `draws` are in the world; the rest go over the UI.
    world_draws: usize,
}

/// GPU side of one texture: a layer's, or a player skin with the mesh it may bring.
struct Look {
    skin: Option<Arc<Skin>>,
    texture: wgpu::BindGroup,
    own_mesh: Option<wgpu::Buffer>,
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
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Float32x3, 3 => Float32x2];
        let pipeline = |entry: &str, target: wgpu::ColorTargetState, write_depth: bool| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                depth_write_enabled: Some(write_depth),
                // Or equal: an entity's later layers lie exactly on its first.
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(target)],
            }),
            multiview_mask: None,
            cache: None,
        });
        // Blended layers leave the depth alone, so what they cover (a slime's core) still shows.
        let blended = wgpu::ColorTargetState { format: color, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::COLOR };
        let (pipeline, blend_pipeline) = (pipeline("fs_main", color.into(), true), pipeline("fs_blend", blended, false));
        let instances = storage_buffer(device, INSTANCES, 64 * size_of::<Instance>() as u64);
        let bones = storage_buffer(device, BONES, 1024 * size_of::<BoneMatrix>() as u64);
        let glint = GlintTextures::new(device);
        let bind_group = Self::bind_group(device, &layout, globals, &instances, &bones, &glint);
        EntityPass {
            pipeline,
            blend_pipeline,
            layout,
            texture_layout,
            sampler: entity_textures::sampler(device),
            instances,
            bones,
            bind_group,
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
        self.bind_group = Self::bind_group(device, &self.layout, globals, &self.instances, &self.bones, &self.glint);
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

    /// Uploads this frame's instances. `light` gives the (block, sky) levels at a world position;
    /// `over` are drawn by [`EntityPass::draw_over`] (over the UI), fully lit.
    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::Buffer,
        entities: impl Iterator<Item = &'a EntityInstance>,
        over: &'a [EntityInstance],
        camera: DVec3,
        glint_scroll: [f32; 2],
        light: impl Fn(DVec3) -> [f32; 2],
    ) {
        self.draws.clear();
        self.world_draws = 0;
        self.looks.retain(|_, look| look.skin.as_ref().is_none_or(|s| Arc::strong_count(s) > 1));
        let mut records = Vec::new();
        let mut bones: Vec<BoneMatrix> = Vec::new();
        let all = entities.map(|e| (e, false)).chain(over.iter().map(|e| (e, true)));
        for (e, layer, over) in all.flat_map(|(e, over)| e.layers.iter().map(move |l| (e, l, over))) {
            let shared = self.models.models().get(layer.model as usize).zip(self.ranges.get(layer.model as usize));
            if shared.is_none() && e.skin.as_ref().is_none_or(|s| s.mesh.is_none()) {
                continue;
            }
            let key = match &e.skin {
                Some(skin) => TextureKey::Skin(Arc::as_ptr(skin) as usize),
                None => TextureKey::Layers(layer.textures, layer.tint.is_some()),
            };
            let look = self.looks.entry(key).or_insert_with(|| {
                let view = match &e.skin {
                    Some(s) => entity_textures::upload(device, queue, s.width, s.height, &s.rgba),
                    None => {
                        let files: Vec<&PathBuf> = layer.textures.iter().filter_map(|&t| self.models.textures().get(t as usize)).collect();
                        entity_textures::load(device, queue, &files, layer.tint.is_some())
                    }
                };
                let own_mesh = e.skin.as_ref().and_then(|s| s.mesh.as_ref()).map(|m| vertex_buffer(device, &m.vertices));
                Look { skin: e.skin.clone(), texture: entity_textures::bind_group(device, &self.texture_layout, &view, &self.sampler), own_mesh }
            });
            let own = e.skin.as_ref().and_then(|s| s.mesh.as_ref()).filter(|_| look.own_mesh.is_some());
            let (range, mesh) = match (own, shared) {
                (Some(m), _) => (0..m.vertices.len() as u32, m),
                (None, Some((model, range))) => (range.clone(), &model.mesh),
                (None, None) => continue,
            };
            // Model space has the entity facing -z with its right at -x: mirror z, then turn.
            let body = e.frame.unwrap_or_else(|| {
                Mat4::from_translation((e.position - camera).as_vec3())
                    * Mat4::from_rotation_y(-e.yaw.to_radians())
                    * Mat4::from_scale(Vec3::new(e.scale, e.scale, -e.scale))
            });
            let first_bone = bones.len() as u32;
            bones.extend(mesh.skin(&e.pose).iter().map(|posed| (body * *posed).to_cols_array_2d()));
            let [block, sky] = if over { [15.0, 0.0] } else { light(e.position + DVec3::Y * 0.5) };
            let tint = layer.tint.map_or([0.0; 4], |[r, g, b]| [r, g, b, 1.0]);
            let texture = e.glint.map_or(0.0, |g| if g == Glint::Item { 1.0 } else { 2.0 });
            // Only a skin's size is known here; armour's scale does not ask for it.
            let [sx, sy] = e.glint.map_or([0.0; 2], |g| g.scale(e.skin.as_ref().map_or([16; 2], |s| [s.width, s.height])));
            let glint = [sx, sy, glint_scroll[0], glint_scroll[1]];
            records.push(Instance { light: [block, sky, f32::from(u8::from(e.hurt)), texture], tint, hidden: layer.hidden, bones: [first_bone, 0, 0, 0], glint });
            self.draws.push((range, key, layer.blend));
            if !over {
                self.world_draws = self.draws.len();
            }
        }
        let grown = upload(device, queue, &mut self.instances, INSTANCES, &records) | upload(device, queue, &mut self.bones, BONES, &bones);
        if grown {
            self.bind_group = Self::bind_group(device, &self.layout, globals, &self.instances, &self.bones, &self.glint);
        }
    }

    /// The world's opaque layers.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0..self.world_draws, false);
    }

    /// The world's blended layers, after everything opaque.
    pub fn draw_blended(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0..self.world_draws, true);
    }

    /// The instances given to [`EntityPass::prepare`] as `over`.
    pub fn draw_over(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, self.world_draws..self.draws.len(), false);
        self.draw_range(pass, self.world_draws..self.draws.len(), true);
    }

    fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, draws: Range<usize>, blended: bool) {
        let Some(vertices) = self.vertices.as_ref().filter(|_| !draws.is_empty()) else { return };
        pass.set_pipeline(if blended { &self.blend_pipeline } else { &self.pipeline });
        pass.set_bind_group(0, &self.bind_group, &[]);
        let chosen = self.draws.iter().enumerate().skip(draws.start).take(draws.len()).filter(|(_, d)| d.2 == blended);
        for (index, (range, key, _)) in chosen {
            let look = &self.looks[key];
            pass.set_bind_group(1, &look.texture, &[]);
            pass.set_vertex_buffer(0, look.own_mesh.as_ref().unwrap_or(vertices).slice(..));
            pass.draw(range.clone(), index as u32..index as u32 + 1);
        }
    }
}
