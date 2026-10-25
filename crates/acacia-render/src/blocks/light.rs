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
    /// `getShadeBrightness` is 0.2, not 1: it darkens the ambient occlusion of faces next to it.
    pub dark: bool,
    /// A cube drawn with ambient occlusion: its model's `ambientocclusion`, and no emission. Model
    /// faces carry their own ([`super::ModelFace::ambient_occlusion`]).
    pub ambient_occlusion: bool,
    /// Its face occlusion shapes as the light engine sees them: an index from 1 into
    /// [`super::BlockTable::light_shapes`], 0 when the engine ignores its shape
    /// (`LightEngine.isEmptyShape`).
    pub shape: u16,
}

/// Java's face occlusion shapes of a block, per Java direction (`DOWN`..`EAST`): a 16x16 grid of
/// the face, bit `v * 16 + u` of the 256, where (u, v) is (x, z) on y faces, (x, y) on z faces and
/// (z, y) on x faces.
pub type FaceShapes = [[u64; 4]; 6];

impl LightFacts {
    /// `isLightPermeable`: ambient occlusion looks past it for the diagonal cell.
    pub fn permeable(&self) -> bool {
        !self.solid_render || self.dampening == 0
    }
}
