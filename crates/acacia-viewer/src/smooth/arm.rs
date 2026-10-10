//! The own player's arms in first person ([`acacia_render::item::arm`]).

use acacia_render::Camera;
use acacia_render::entity::EntityInstance;
use acacia_render::item::arm::{self, Wearer};
use acacia_render::item::map::Hold;

use super::Smoother;

impl Smoother {
    /// `None` until the own player is drawn.
    fn own(&self) -> Option<Wearer<'_>> {
        let (own, _) = self.to.iter().find(|(e, _)| e.own_eyes.is_some())?;
        let model = own.instance.layers.first().and_then(|l| self.models.models().get(l.model as usize)).map(|model| &model.mesh);
        let mesh = own.instance.skin.as_ref().and_then(|skin| skin.mesh.as_ref()).or(model)?;
        Some(Wearer { layers: &own.instance.layers, skin: own.instance.skin.clone(), bones: &mesh.bones })
    }

    /// The bare right arm; `swing` is 0 to 1 through an arm swing.
    pub fn own_arm(&self, camera: &Camera, swing: f32) -> Option<EntityInstance> {
        Some(arm::first_person(&self.own()?, camera, swing))
    }

    /// The arms holding a map as `hold`.
    pub fn map_arms(&self, camera: &Camera, hold: Hold) -> Vec<EntityInstance> {
        self.own().map_or_else(Vec::new, |own| arm::with_map(&own, camera, hold))
    }
}
