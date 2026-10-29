//! Draws [`EntityInstance`]s: one vertex buffer holding every model, one instance record per
//! layer of an entity, one bind group per texture (a layer's composed textures or a player skin).

mod pipelines;
mod setup;

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::{DVec3, Mat4, Vec3};

use super::entity_buffers::{BONES, INSTANCES, upload, vertex_buffer};
use super::entity_textures;
use super::glint::GlintTextures;
use crate::entity::{Blend, EntityInstance, EntityModels, Skin, TextureId, Vertex};
use crate::glint::Glint;

/// A swirl's texture slides by this share of itself per tick, both ways (Java's
/// `EnergySwirlLayer` for the creeper; the pack's `uv_anim` says the same and is not read).
const SWIRL_PER_SEC: f32 = 0.01 * 20.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    /// x: block light, y: sky light (0..=15), z: the overlay (`overlay::keep`), w: the glint texture
    /// (0 none, 1 the item's, 2 the armour's).
    light: [f32; 4],
    /// rgb: the layer's tint, a: 1 when the texture's alpha is a tint mask and not a cutout.
    tint: [f32; 4],
    hidden: [u32; 4],
    /// x: index of the layer's first matrix in the bone buffer, y: the bits of the layer's alpha
    /// ([`crate::entity::Fade`]).
    bones: [u32; 4],
    /// xy: the glint's scale over the UVs, zw: how far it has slid ([`crate::glint`]); on a
    /// [`Blend::Swirl`] or [`Blend::Wind`] layer, how far its texture has.
    glint: [f32; 4],
}

/// Model space to camera-relative world space, one per bone of each instance.
type BoneMatrix = [[f32; 4]; 4];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextureKey {
    /// A layer's textures, whether they compose as a tint mask, and its leather dye.
    Layers([TextureId; 3], bool, Option<[u8; 3]>),
    /// Address of the shared [`Skin`]; the map keeps the `Arc` alive, so it stays unique.
    Skin(usize),
}

pub struct EntityPass {
    /// By [`pipelines::index`] of a layer's [`Blend`].
    pipelines: [wgpu::RenderPipeline; pipelines::BLENDS.len()],
    /// The clock a swirl scrolls and a layer fades by.
    started: Instant,
    layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instances: wgpu::Buffer,
    bones: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The globals the instances over the UI are drawn with ([`EntityPass::set_over_globals`]):
    /// pictures-in-picture have a projection and lights of their own.
    over_globals: wgpu::Buffer,
    over_bind_group: wgpu::BindGroup,
    pub glint: GlintTextures,
    models: Arc<EntityModels>,
    vertices: Option<wgpu::Buffer>,
    ranges: Vec<Range<u32>>,
    looks: HashMap<TextureKey, Look>,
    /// Vertices, texture and how the layer is drawn, per instance record.
    draws: Vec<(Range<u32>, TextureKey, Blend)>,
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
    #[allow(clippy::too_many_arguments)]
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
        let elapsed = self.started.elapsed().as_secs_f32();
        let swirl = (elapsed * SWIRL_PER_SEC).fract();
        let all = entities.map(|e| (e, false)).chain(over.iter().map(|e| (e, true)));
        for (e, layer, over) in all.flat_map(|(e, over)| e.layers.iter().map(move |l| (e, l, over))) {
            let alpha = layer.fade.alpha(elapsed * 20.0);
            let shared = self.models.models().get(layer.model as usize).zip(self.ranges.get(layer.model as usize));
            if alpha <= 1e-5 || (shared.is_none() && e.skin.as_ref().is_none_or(|s| s.mesh.is_none())) {
                continue;
            }
            let key = match &e.skin {
                Some(skin) => TextureKey::Skin(Arc::as_ptr(skin) as usize),
                None => TextureKey::Layers(layer.textures, layer.tint.is_some(), layer.dye),
            };
            let look = self.looks.entry(key).or_insert_with(|| {
                let view = match &e.skin {
                    Some(s) => entity_textures::upload(device, queue, s.width, s.height, &s.rgba),
                    None => {
                        let files: Vec<&PathBuf> = layer.textures.iter().filter_map(|&t| self.models.textures().get(t as usize)).collect();
                        entity_textures::load(device, queue, &files, layer.tint.is_some(), layer.dye)
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
            // Java lights a GUI entity at full block and sky light (15728880).
            let [block, sky] = if over { [15.0, 15.0] } else { light(e.position + DVec3::Y * 0.5) };
            let tint = layer.tint.map_or([0.0; 4], |[r, g, b]| [r, g, b, 1.0]);
            let texture = e.glint.map_or(0.0, |g| if g == Glint::Item { 1.0 } else { 2.0 });
            // Only a skin's size is known here; armour's scale does not ask for it.
            let [sx, sy] = e.glint.map_or([0.0; 2], |g| g.scale(e.skin.as_ref().map_or([16; 2], |s| [s.width, s.height])));
            let slide = match layer.blend {
                Blend::Swirl => [swirl; 2],
                // BreezeWindLayer slides twice as fast as a swirl, sideways only.
                Blend::Wind => [(swirl * 2.0).fract(), 0.0],
                _ => glint_scroll,
            };
            let glint = [sx, sy, slide[0], slide[1]];
            let light = [block, sky, crate::entity::java::overlay::keep(e.hurt, e.white), texture];
            records.push(Instance { light, tint, hidden: layer.hidden, bones: [first_bone, alpha.to_bits(), 0, 0], glint });
            self.draws.push((range, key, layer.blend));
            if !over {
                self.world_draws = self.draws.len();
            }
        }
        let grown = upload(device, queue, &mut self.instances, INSTANCES, &records) | upload(device, queue, &mut self.bones, BONES, &bones);
        if grown {
            self.rebind(device, globals);
        }
    }

    /// The world's opaque layers, then the depth-only ones, before anything translucent.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0..self.world_draws, Blend::Opaque, &self.bind_group);
        self.draw_range(pass, 0..self.world_draws, Blend::Mask, &self.bind_group);
    }

    /// The world's blended layers, after everything opaque.
    pub fn draw_blended(&self, pass: &mut wgpu::RenderPass<'_>) {
        for blend in &pipelines::BLENDS[2..] {
            self.draw_range(pass, 0..self.world_draws, *blend, &self.bind_group);
        }
    }

    /// The instances given to [`EntityPass::prepare`] as `over`, with the over globals.
    pub fn draw_over(&self, pass: &mut wgpu::RenderPass<'_>) {
        for blend in pipelines::BLENDS {
            self.draw_range(pass, self.world_draws..self.draws.len(), blend, &self.over_bind_group);
        }
    }

    fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, draws: Range<usize>, blend: Blend, bind_group: &wgpu::BindGroup) {
        let Some(vertices) = self.vertices.as_ref().filter(|_| !draws.is_empty()) else { return };
        pass.set_pipeline(&self.pipelines[pipelines::index(blend)]);
        pass.set_bind_group(0, bind_group, &[]);
        let chosen = self.draws.iter().enumerate().skip(draws.start).take(draws.len()).filter(|(_, d)| d.2 == blend);
        for (index, (range, key, _)) in chosen {
            let look = &self.looks[key];
            pass.set_bind_group(1, &look.texture, &[]);
            pass.set_vertex_buffer(0, look.own_mesh.as_ref().unwrap_or(vertices).slice(..));
            pass.draw(range.clone(), index as u32..index as u32 + 1);
        }
    }
}
