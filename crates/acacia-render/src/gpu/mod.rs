mod atlas;
mod device;
mod entities;
mod entity_textures;
mod globals;
mod pipeline;
mod screenshot;
mod sky;
mod store;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use acacia_world::World;
use glam::DVec3;

use crate::block_models::{BlockDataMap, BlockModels};
use crate::entity::{EntityInstance, EntityModels};
use crate::sky::SkyTextures;
use entities::EntityPass;
use sky::SkyPass;

use crate::Error;
use crate::assets::flipbook::Atlas;
use crate::assets::image::Texture;
use crate::blocks::BlockTable;
use atlas::BlockTextures;
use crate::biome::BiomeColors;
use crate::camera::{Camera, Frustum};
use crate::cull;
use crate::light::Lighting;
use crate::look::Look;
use crate::scene::{Scene, Update};
use crate::sky::{NOON, Sky};
use globals::{Globals, srgb_to_linear};
use pipeline::Pipelines;
use store::Store;

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub sections: usize,
    pub drawn: usize,
    pub quads: u64,
    pub gpu_bytes: u64,
    /// Sections waiting for or being meshed.
    pub pending: usize,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    pipelines: Pipelines,
    globals: wgpu::Buffer,
    textures: BlockTextures,
    /// Texture animations run on game ticks counted from here.
    started: Instant,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    store: Store,
    entities: EntityPass,
    entity_list: Vec<EntityInstance>,
    block_models: BlockModels,
    /// `None` until [`Renderer::set_sky_textures`]: the sky is then a plain colour.
    sky: Option<SkyPass>,
    scene: Option<Scene>,
    biomes: Arc<BiomeColors>,
    updates: Vec<Update>,
    /// Blocks from the camera where fog turns opaque.
    pub fog_distance: f32,
    /// Skip sections no open path from the camera reaches ([`crate::cull`]).
    pub cave_culling: bool,
    /// Time of day in ticks ([`crate::sky`]); noon until set.
    pub time: f32,
    /// [`crate::sky::moon_phase`]; full until set.
    pub moon_phase: u8,
    pub look: Look,
    screenshot: Option<PathBuf>,
}

impl Renderer {
    pub fn new(target: impl Into<wgpu::SurfaceTarget<'static>>, size: (u32, u32)) -> Result<Renderer, Error> {
        let device::Gpu { surface, device, queue, config } = device::open(target, size)?;
        let pipelines = Pipelines::new(&device, config.format);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let textures = BlockTextures::new(&device, &queue, &Atlas { layers: vec![Texture::missing()], animations: Vec::new() });
        let sampler = pipeline::sampler(&device);
        let store = Store::new(&device);
        let entities = EntityPass::new(&device, config.format, &globals);
        let bind_group = pipeline::bind_group(&device, &pipelines.layout, &globals, &store, &textures.view, &sampler);
        Ok(Renderer {
            depth: pipeline::depth_view(&device, config.width, config.height),
            surface,
            device,
            queue,
            config,
            pipelines,
            globals,
            textures,
            started: Instant::now(),
            sampler,
            bind_group,
            store,
            entities,
            entity_list: Vec::new(),
            block_models: BlockModels::default(),
            sky: None,
            scene: None,
            biomes: Arc::default(),
            updates: Vec::new(),
            fog_distance: 160.0,
            cave_culling: true,
            time: NOON,
            moon_phase: 0,
            look: Look::default(),
            screenshot: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        (self.config.width, self.config.height) = (width, height);
        self.surface.configure(&self.device, &self.config);
        self.depth = pipeline::depth_view(&self.device, width, height);
    }

    pub fn set_vsync(&mut self, on: bool) {
        self.config.present_mode = if on { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        self.surface.configure(&self.device, &self.config);
    }

    /// Saves the next frame as a PNG (needs a surface that allows copies; logged otherwise).
    pub fn screenshot(&mut self, path: PathBuf) {
        self.screenshot = Some(path);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Starts drawing a world, dropping the previous one's meshes. `table` and `atlas` come from a
    /// [`crate::LookPack`] over the world's registry (custom blocks shift runtime ids).
    pub fn set_world(&mut self, world: Arc<World>, table: Arc<BlockTable>, atlas: &Atlas) {
        self.textures = BlockTextures::new(&self.device, &self.queue, atlas);
        self.store.replaced = true;
        let lighting = Lighting::new(world.clone());
        self.rebuild_scene(world, table, lighting);
    }

    /// Replaces the biome colours (from the server's `BiomeDefinitionList`) and remeshes.
    pub fn set_biomes(&mut self, biomes: Arc<BiomeColors>) {
        self.biomes = biomes;
        if let Some(scene) = self.scene.take() {
            let (world, table, lighting) = scene.into_parts();
            self.rebuild_scene(world, table, lighting);
        }
    }

    fn rebuild_scene(&mut self, world: Arc<World>, table: Arc<BlockTable>, lighting: Lighting) {
        self.store.clear();
        self.block_models.clear();
        self.updates.clear();
        self.scene = Some(Scene::new(world, table, self.biomes.clone(), lighting));
    }

    pub fn set_entity_models(&mut self, models: Arc<EntityModels>) {
        self.block_models.set_models(models.clone());
        self.entities.set_models(&self.device, models);
    }

    /// What the block entities add to their blocks' models (bed colours, chest pairs).
    pub fn set_block_data(&mut self, data: Arc<BlockDataMap>) {
        self.block_models.set_data(data);
    }

    /// Draws the sun, moon and stars from now on.
    pub fn set_sky_textures(&mut self, textures: &SkyTextures) {
        self.sky = Some(SkyPass::new(&self.device, &self.queue, self.config.format, &self.globals, textures));
    }

    /// The entities to draw from now on; ids come from the models set last.
    pub fn set_entities(&mut self, entities: Vec<EntityInstance>) {
        self.entity_list = entities;
    }

    pub fn world(&self) -> Option<&Arc<World>> {
        self.scene.as_ref().map(Scene::world)
    }

    pub fn render(&mut self, camera: &Camera) -> FrameStats {
        let (cam_block, cam_frac) = camera.split_position();
        if let Some(scene) = &mut self.scene {
            scene.pump(cam_block, &mut self.updates);
        }
        for update in self.updates.drain(..) {
            match update {
                Update::Mesh(key, mut mesh) => {
                    self.block_models.set_section(key, std::mem::take(&mut mesh.models));
                    self.store.upload(&self.device, &self.queue, key, mesh);
                }
                Update::Light(key, light) => self.store.upload_light(&self.queue, key, &light),
                Update::Remove(key) => {
                    self.block_models.set_section(key, Vec::new());
                    self.store.remove(key);
                }
            }
        }
        if std::mem::take(&mut self.store.replaced) {
            self.bind_group = pipeline::bind_group(&self.device, &self.pipelines.layout, &self.globals, &self.store, &self.textures.view, &self.sampler);
        }
        self.textures.animate(&self.queue, (self.started.elapsed().as_secs_f64() * 20.0) as u64);

        let view_proj = camera.view_proj();
        let has_sky = self.world().is_none_or(|w| w.dimension().sky);
        let sky = Sky::at(if has_sky { self.time } else { NOON });
        let sky_pass = self.sky.as_ref().filter(|_| has_sky);
        if let Some(pass) = sky_pass {
            pass.prepare(&self.queue, &sky, self.moon_phase);
        }
        let sky_color = srgb_to_linear(sky.color);
        // Only the Nether is that low; the End hazes as the overworld does.
        let nether = self.world().is_some_and(|w| !w.dimension().sky && w.dimension().height <= 128);
        let haze = self.look.fog.haze.map(|h| if nether { h.nether } else { h.overworld });
        let globals = Globals::new(view_proj, (cam_block, cam_frac), sky_color, self.fog_distance, &self.look, haze, has_sky, sky.darken);
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let light = self.scene.as_ref().map(|s| s.light().read());
        // Outside lit columns an entity is as bright as open sky.
        let light_at = |p: DVec3| {
            let p = p.floor().as_ivec3();
            let byte = light.as_ref().and_then(|l| l.light(p.x, p.y, p.z)).unwrap_or(15);
            [f32::from(byte >> 4), f32::from(byte & 15)]
        };
        let blocks = self.block_models.near(camera.position, f64::from(self.fog_distance));
        self.entities.prepare(&self.device, &self.queue, &self.globals, self.entity_list.iter().chain(blocks), camera.position, light_at);
        drop(light);
        let frustum = Frustum::new(view_proj);
        let reachable = self
            .scene
            .as_ref()
            .filter(|_| self.cave_culling)
            .map(|s| cull::visible_sections(&frustum, cam_block, cam_frac, s.sections(), |k| s.visibility(k)));
        let (solid, translucent) = self.store.draws(&frustum, cam_block, cam_frac, reachable.as_ref());
        let stats = FrameStats {
            sections: self.store.sections(),
            drawn: solid.len().max(translucent.len()),
            quads: self.store.used_quads(),
            gpu_bytes: self.store.gpu_bytes(),
            pending: self.scene.as_ref().map_or(0, Scene::pending),
        };

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                self.surface.configure(&self.device, &self.config);
                f
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return stats;
            }
            _ => return stats,
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = pipeline::begin_pass(&mut encoder, &view, &self.depth, sky_color);
            if let Some(sky) = sky_pass {
                sky.draw(&mut pass);
            }
            for (pipeline, draws) in [(&self.pipelines.solid, &solid), (&self.pipelines.translucent, &translucent)] {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                for d in draws {
                    pass.draw(d.first * 6..(d.first + d.count) * 6, d.slot..d.slot + 1);
                }
                if std::ptr::eq(pipeline, &self.pipelines.solid) {
                    self.entities.draw(&mut pass);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        if let Some(path) = self.screenshot.take() {
            match screenshot::save(&self.device, &self.queue, &frame.texture, &path) {
                Ok(()) => tracing::info!(path = %path.display(), "screenshot"),
                Err(e) => tracing::warn!(%e, "screenshot failed"),
            }
        }
        self.queue.present(frame);
        stats
    }
}
