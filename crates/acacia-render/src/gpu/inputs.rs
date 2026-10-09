//! What the caller hands the renderer between frames: the world, its colours and models, entities,
//! the sky textures and the targeted block.

use std::path::PathBuf;
use std::sync::Arc;

use acacia_world::World;

use super::Renderer;
use super::atlas::BlockTextures;
use super::outline::Outline;
use super::sky::SkyPass;
use crate::assets::flipbook::Atlas;
use crate::biome::BiomeColors;
use crate::block_models::BlockDataMap;
use crate::blocks::BlockTable;
use crate::entity::{EntityInstance, EntityModels};
use crate::light::Lighting;
use crate::scene::Scene;
use crate::sky::SkyTextures;

impl Renderer {
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        (self.config.width, self.config.height) = (width, height);
        self.surface.configure(&self.device, &self.config);
        self.depth = super::pipeline::depth_view(&self.device, width, height);
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

    /// The text on signs, laid out in the UI atlas given to [`Renderer::render`].
    pub fn set_sign_text(&mut self, text: Arc<crate::sign_text::SignTextMap>) {
        self.block_models.set_sign_text(text);
    }

    /// Draws the sun, moon and stars from now on.
    pub fn set_sky_textures(&mut self, textures: &SkyTextures) {
        self.sky = Some(SkyPass::new(&self.device, &self.queue, self.config.format, &self.globals, textures));
    }

    /// The entities to draw from now on; ids come from the models set last.
    pub fn set_entities(&mut self, entities: Vec<EntityInstance>) {
        self.entity_list = entities;
    }

    /// The destroy stages drawn on a block being mined, from [`super::load_crack_stages`].
    pub fn set_crack_stages(&mut self, strip: &image::RgbaImage) {
        self.crack_pass.set_stages(&self.device, &self.queue, strip);
    }

    /// The rain streaks, from [`crate::weather::load_texture`].
    pub fn set_weather_texture(&mut self, streaks: &crate::weather::Streaks) {
        self.weather_pass.set_texture(&self.device, &self.queue, streaks);
    }

    /// The cloud map, from [`crate::weather::load_clouds`].
    pub fn set_cloud_texture(&mut self, image: &image::RgbaImage) {
        self.cloud_pass.set_texture(image);
    }

    /// The overlay drawn with the camera in water, from [`crate::fluid_view::load_underwater`].
    pub fn set_underwater_texture(&mut self, image: &image::RgbaImage) {
        self.screen_effect.set_texture(&self.device, &self.queue, image);
    }

    /// The block outlined from now on, or none.
    pub fn set_outline(&mut self, outline: Option<Outline>) {
        self.outline = outline;
    }

    pub fn world(&self) -> Option<&Arc<World>> {
        self.scene.as_ref().map(Scene::world)
    }
}
