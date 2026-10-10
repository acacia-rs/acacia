//! The own player's bare arm in first person ([`acacia_render::item::arm`]).

use acacia_render::Camera;
use acacia_render::entity::EntityInstance;
use acacia_render::item::arm;

use super::Smoother;

impl Smoother {
    /// `None` until the own player is drawn; `swing` is 0 to 1 through an arm swing.
    pub fn own_arm(&self, camera: &Camera, swing: f32) -> Option<EntityInstance> {
        let (own, _) = self.to.iter().find(|(e, _)| e.own_eyes.is_some())?;
        let model = own.instance.layers.first().and_then(|l| self.models.models().get(l.model as usize)).map(|model| &model.mesh);
        let mesh = own.instance.skin.as_ref().and_then(|skin| skin.mesh.as_ref()).or(model)?;
        Some(arm::first_person(&own.instance.layers, own.instance.skin.clone(), &mesh.bones, camera, swing))
    }
}
