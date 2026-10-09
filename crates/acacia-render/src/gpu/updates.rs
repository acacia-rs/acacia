//! What the scene's workers finished since the last frame, taken into the GPU's stores.

use glam::IVec3;

use super::{Renderer, pipeline};
use crate::scene::Update;

impl Renderer {
    /// Uploads finished meshes and light, and drops removed sections, for a camera in `cam_block`.
    pub(super) fn take_updates(&mut self, cam_block: IVec3) {
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
    }
}
