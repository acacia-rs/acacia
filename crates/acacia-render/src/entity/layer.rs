//! One draw of an entity, and what the pack's material names mean to the entity pass (no
//! material file is read). See README "Entities".

use super::{ModelId, NO_TEXTURE, TextureId};

/// One draw of an entity: a render controller's choice of mesh and textures.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub model: ModelId,
    /// Laid over each other, bottom first; unused slots hold [`NO_TEXTURE`].
    pub textures: [TextureId; 3],
    /// Linear colour multiplied in where the texture's alpha is 0 (sheep wool).
    pub tint: Option<[f32; 3]>,
    /// Bit per bone of [`super::Mesh::bones`] that is not drawn.
    pub hidden: [u32; 4],
    pub blend: Blend,
}

/// How a layer goes onto the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blend {
    #[default]
    Opaque,
    /// Over what is behind by the texture's alpha (a slime's shell), after the opaque layers.
    Alpha,
    /// Added to the frame, unlit, its texture sliding (a charged creeper's aura).
    Swirl,
}

/// Materials blended by their texture's alpha: the shells of slimes and sulfur cubes.
const BLENDED_MATERIALS: [&str; 1] = ["outer"];
const SWIRL_MATERIALS: [&str; 1] = ["charged"];
/// Materials with effects the entity pass does not draw (the guardian's ghost): their layers are
/// left out. `enchanted` layers too: the glint is the instance's ([`super::EntityInstance::glint`]).
const OVERLAY_MATERIALS: [&str; 6] = ["ghost", "wind", "bioluminescent", "dissolve", "spectator", "enchanted"];

impl Blend {
    /// For a layer of `material`; `None` for one that is not drawn.
    pub(super) fn of(material: &str) -> Option<Blend> {
        let any = |names: &[&str]| names.iter().any(|m| material.contains(m));
        if any(&OVERLAY_MATERIALS) {
            return None;
        }
        Some(if any(&SWIRL_MATERIALS) { Blend::Swirl } else if any(&BLENDED_MATERIALS) { Blend::Alpha } else { Blend::Opaque })
    }
}

impl Layer {
    /// `model` in one texture, opaque.
    pub fn plain(model: ModelId, texture: TextureId) -> Layer {
        Layer { model, textures: [texture, NO_TEXTURE, NO_TEXTURE], tint: None, hidden: [0; 4], blend: Blend::Opaque }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materials_pick_how_a_layer_blends() {
        assert_eq!(Blend::of("creeper"), Some(Blend::Opaque));
        assert_eq!(Blend::of("charged_creeper"), Some(Blend::Swirl));
        assert_eq!(Blend::of("slime_outer"), Some(Blend::Alpha));
        assert_eq!((Blend::of("guardian_ghost"), Blend::of("armor_enchanted")), (None, None));
    }
}
