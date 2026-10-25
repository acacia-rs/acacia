//! What a block is to light under [`crate::look::Lighting::Java`]: the facts of the Java state it
//! maps to, baked by `lookbake java` and checked against the game.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LightFacts {
    /// `getLightEmission`.
    pub emission: u8,
    /// `getLightDampening`: levels a step into the block loses past the first.
    pub dampening: u8,
    /// `isSolidRender`.
    pub solid_render: bool,
    /// `isCollisionShapeFullBlock`.
    pub full_collision: bool,
    /// `emissiveRendering`: drawn at full brightness whatever the light.
    pub emissive: bool,
    /// A cube drawn with ambient occlusion: its model's `ambientocclusion`, and no emission. Model
    /// faces carry their own ([`super::ModelFace::ambient_occlusion`]).
    pub ambient_occlusion: bool,
}

impl LightFacts {
    /// `isLightPermeable`: ambient occlusion looks past it for the diagonal cell.
    pub fn permeable(&self) -> bool {
        !self.solid_render || self.dampening == 0
    }
}
