//! The look packs the viewer switches between.

use std::sync::Arc;

use acacia_render::assets::Pack;
use acacia_render::{Error, Look, LookPack};

use crate::settings::LookChoice;

pub struct Looks {
    bedrock: Arc<LookPack>,
    java: Arc<LookPack>,
}

impl Looks {
    /// Packs baked by tools/lookbake where present. Without one, the Bedrock look is baked here
    /// from the resource pack ([`Pack::default_dir`]), and the Java look is the Bedrock one with
    /// Java's parameters until it has assets of its own.
    pub fn load() -> Result<Looks, Error> {
        let bedrock = match baked("bedrock") {
            Some(pack) => pack,
            None => {
                let (baked, report) = LookPack::bake_bedrock(&Pack::load(&Pack::default_dir())?, Look::BEDROCK);
                let (untextured, missing) = (report.blocks_without_textures.len(), report.missing_images.len());
                tracing::info!(textures = baked.atlas.layers.len(), animated = baked.atlas.animations.len(), untextured, missing, "baked the Bedrock look");
                tracing::debug!(untextured = ?report.blocks_without_textures, missing = ?report.missing_images);
                baked
            }
        };
        let java = baked("java").unwrap_or_else(|| bedrock.clone().with_look(Look::JAVA));
        Ok(Looks { bedrock: Arc::new(bedrock), java: Arc::new(java) })
    }

    pub fn get(&self, choice: LookChoice) -> &Arc<LookPack> {
        match choice {
            LookChoice::Bedrock => &self.bedrock,
            LookChoice::Java => &self.java,
        }
    }
}

/// `None` when the look has no baked pack, or one this build cannot read (logged).
fn baked(name: &str) -> Option<LookPack> {
    let dir = LookPack::default_dir(name);
    if !dir.join("pack.json").is_file() {
        return None;
    }
    match LookPack::load(&dir) {
        Ok(pack) => {
            tracing::info!(dir = %dir.display(), "look pack");
            Some(pack)
        }
        Err(e) => {
            tracing::warn!(%e, "look pack ignored");
            None
        }
    }
}
