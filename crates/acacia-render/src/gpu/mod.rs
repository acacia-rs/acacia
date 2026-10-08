mod atlas;
mod bolts;
mod clouds;
mod crack;
mod device;
mod entities;
mod entity_textures;
mod globals;
mod inputs;
mod outline;
mod particles;
mod pipeline;
mod screenshot;
mod sky;
mod store;
mod ui;
mod weather;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::DVec3;

use crate::block_models::BlockModels;
use crate::entity::EntityInstance;
use entities::EntityPass;
use crack::CrackPass;
pub use crack::load_stages as load_crack_stages;
use outline::OutlinePass;
use ui::UiPass;
pub use outline::Outline;
use sky::SkyPass;

use crate::Error;
use crate::assets::flipbook::Atlas;
use crate::assets::image::Texture;
use atlas::BlockTextures;
use crate::biome::BiomeColors;
use crate::camera::{Camera, Frustum};
use crate::cull;
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
    outline_pass: OutlinePass,
    outline: Option<Outline>,
    crack_pass: CrackPass,
    ui_pass: UiPass,
    particle_pass: particles::ParticlePass,
    particles: crate::particles::Particles,
    /// The game tick the particles were last advanced to.
    particle_tick: u64,
    weather_pass: weather::WeatherPass,
    /// Rain, thunder and lightning: the falling streaks and the sky.
    pub weather: crate::sky::Weather,
    cloud_pass: clouds::CloudPass,
    bolt_pass: bolts::BoltPass,
    /// Lightning bolts: seed and where each strikes.
    pub bolts: Vec<(u64, DVec3)>,
    /// World y of the cloud layer; `None` draws none.
    pub cloud_height: Option<f32>,
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
        let outline_pass = OutlinePass::new(&device, config.format, &globals);
        let ui_pass = UiPass::new(&device, config.format.remove_srgb_suffix());
        let particle_pass = particles::ParticlePass::new(&device, config.format);
        let weather_pass = weather::WeatherPass::new(&device, config.format, &globals);
        let cloud_pass = clouds::CloudPass::new(&device, config.format, &globals);
        let bolt_pass = bolts::BoltPass::new(&device, config.format, &globals);
        let crack_pass = CrackPass::new(&device, config.format, &globals);
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
            outline_pass,
            outline: None,
            crack_pass,
            ui_pass,
            particle_pass,
            particles: Default::default(),
            particle_tick: 0,
            weather_pass,
            weather: Default::default(),
            cloud_pass,
            cloud_height: None,
            bolt_pass,
            bolts: Vec::new(),
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

    /// Draws a frame from `camera`, with `ui` (its atlas and this frame's quads) over it.
    pub fn render(&mut self, camera: &Camera, ui: Option<(&acacia_ui::Atlas, &[acacia_ui::Quad])>) -> FrameStats {
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
        self.prepare_particles(camera);
        let world = self.scene.as_ref().map(|s| s.world().clone());
        self.weather_pass.prepare(&self.queue, world.as_deref(), &self.biomes, camera.position, self.weather.rain, self.started.elapsed().as_secs_f32());
        let clouds = self.cloud_height.filter(|_| world.as_ref().is_none_or(|w| w.dimension().sky));
        self.cloud_pass.prepare(&self.device, &self.queue, camera.position, clouds, crate::sky::cloud_tint(self.weather), self.started.elapsed().as_secs_f64());
        self.bolt_pass.prepare(&self.device, &self.queue, &self.bolts, camera.position);

        let view_proj = camera.view_proj();
        let has_sky = self.world().is_none_or(|w| w.dimension().sky);
        let sky = match has_sky {
            true => Sky::at(self.time, self.weather),
            false => Sky::at(NOON, Default::default()),
        };
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
        self.outline_pass.prepare(&self.queue, self.outline.as_ref(), camera.position);
        self.crack_pass.prepare(&self.queue, self.outline.as_ref(), camera.position);
        let (atlas, quads) = ui.map_or((None, &[][..]), |(a, q)| (Some(a), q));
        if let Some(atlas) = atlas {
            self.ui_pass.prepare(&self.device, &self.queue, atlas, quads, [self.config.width, self.config.height]);
        }
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
            self.particle_pass.draw(&self.device, &mut pass, &self.globals, &self.textures.view, &self.sampler);
            self.cloud_pass.draw(&mut pass);
            self.bolt_pass.draw(&mut pass);
            self.weather_pass.draw(&mut pass);
            self.crack_pass.draw(&mut pass);
            self.outline_pass.draw(&mut pass);
        }
        if atlas.is_some() {
            self.ui_pass.draw(&mut encoder, &frame.texture);
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
