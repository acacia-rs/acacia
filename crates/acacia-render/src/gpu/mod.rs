mod pipeline;
mod screenshot;
mod store;

use std::path::PathBuf;
use std::sync::Arc;

use acacia_world::World;

use crate::Error;
use crate::assets::image::Texture;
use crate::blocks::BlockTable;
use crate::biome::BiomeColors;
use crate::blocks::tint::WATER_ALPHA;
use crate::camera::{Camera, Frustum};
use crate::light::Lighting;
use crate::scene::{Scene, Update};
use pipeline::Pipelines;
use store::Store;

/// Sky and fog colour (sRGB).
const SKY: [f32; 3] = [0.62, 0.76, 1.0];
/// Brightness of unlit blocks, with and without sky light (Java's nether ambient is 0.1).
const AMBIENT: (f32, f32) = (0.02, 0.1);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_block: [i32; 4],
    cam_frac: [f32; 4],
    water: [f32; 4],
    fog: [f32; 4],
    /// x: ambient brightness.
    light: [f32; 4],
}

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
    textures: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    store: Store,
    scene: Option<Scene>,
    biomes: Arc<BiomeColors>,
    updates: Vec<Update>,
    /// Blocks from the camera where fog turns opaque.
    pub fog_distance: f32,
    screenshot: Option<PathBuf>,
}

impl Renderer {
    pub fn new(target: impl Into<wgpu::SurfaceTarget<'static>>, (width, height): (u32, u32)) -> Result<Renderer, Error> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(target).map_err(|e| Error::Surface(e.to_string()))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| Error::Adapter(e.to_string()))?;
        tracing::info!(adapter = ?adapter.get_info().name, backend = ?adapter.get_info().backend, "gpu");
        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("acacia"),
            required_limits: wgpu::Limits {
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                max_texture_array_layers: limits.max_texture_array_layers,
                ..wgpu::Limits::default()
            },
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| Error::Device(e.to_string()))?;
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or_else(|| Error::Surface("surface unsupported by adapter".into()))?;
        config.format = config.format.add_srgb_suffix();
        if surface.get_capabilities(&adapter).usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let pipelines = Pipelines::new(&device, config.format);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture_view = pipeline::texture_array(&device, &queue, &[Texture::missing()]);
        let sampler = pipeline::sampler(&device);
        let store = Store::new(&device);
        let bind_group = bind_group(&device, &pipelines.layout, &globals, &store, &texture_view, &sampler);
        Ok(Renderer {
            depth: pipeline::depth_view(&device, config.width, config.height),
            surface,
            device,
            queue,
            config,
            pipelines,
            globals,
            textures: texture_view,
            sampler,
            bind_group,
            store,
            scene: None,
            biomes: Arc::default(),
            updates: Vec::new(),
            fog_distance: 160.0,
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

    /// Starts drawing a world, dropping the previous one's meshes. `table` and `textures` come from
    /// [`BlockTable::build`] over the world's registry (custom blocks shift runtime ids).
    pub fn set_world(&mut self, world: Arc<World>, table: Arc<BlockTable>, textures: &[Texture]) {
        self.textures = pipeline::texture_array(&self.device, &self.queue, textures);
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
        self.updates.clear();
        self.scene = Some(Scene::new(world, table, self.biomes.clone(), lighting));
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
                Update::Mesh(key, mesh) => self.store.upload(&self.device, &self.queue, key, mesh),
                Update::Light(key, light) => self.store.upload_light(&self.queue, key, &light),
                Update::Remove(key) => self.store.remove(key),
            }
        }
        if std::mem::take(&mut self.store.replaced) {
            self.bind_group = bind_group(&self.device, &self.pipelines.layout, &self.globals, &self.store, &self.textures, &self.sampler);
        }

        let view_proj = camera.view_proj();
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_block: [cam_block.x, cam_block.y, cam_block.z, 0],
            cam_frac: [cam_frac.x, cam_frac.y, cam_frac.z, 0.0],
            water: [WATER_ALPHA, 0.0, 0.0, 0.0],
            fog: { let s = srgb_to_linear(SKY); [s[0], s[1], s[2], self.fog_distance] },
            light: [if self.world().is_none_or(|w| w.dimension().sky) { AMBIENT.0 } else { AMBIENT.1 }, 0.0, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let (solid, translucent) = self.store.draws(&Frustum::new(view_proj), cam_block, cam_frac);
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
            let sky = srgb_to_linear(SKY).map(f64::from);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: sky[0], g: sky[1], b: sky[2], a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            for (pipeline, draws) in [(&self.pipelines.solid, &solid), (&self.pipelines.translucent, &translucent)] {
                pass.set_pipeline(pipeline);
                for d in draws {
                    pass.draw(d.first * 6..(d.first + d.count) * 6, d.slot..d.slot + 1);
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

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    store: &Store,
    textures: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: store.quads.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: store.origins.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(textures) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(sampler) },
            wgpu::BindGroupEntry { binding: 5, resource: store.light.as_entire_binding() },
        ],
    })
}

fn srgb_to_linear(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) })
}
