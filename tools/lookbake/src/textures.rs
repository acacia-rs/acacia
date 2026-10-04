//! Java textures (`textures/<id>.png`, animated by a `.png.mcmeta` beside it) as layers of a look
//! pack's texture array.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use acacia_render::assets::flipbook::{Animation, Atlas, Flipbook};
use acacia_render::assets::image::{Alpha, Texture};
use serde_json::Value;

/// The quad format's layer index is 12 bits.
const MAX_LAYERS: usize = 4096;

pub struct Textures {
    /// The jar's `assets/minecraft`.
    assets: PathBuf,
    loaded: HashMap<String, Option<(u16, Alpha)>>,
    /// Texture ids with no readable image.
    pub missing: BTreeSet<String>,
}

impl Textures {
    pub fn new(assets: PathBuf) -> Textures {
        Textures { assets, loaded: HashMap::new(), missing: BTreeSet::new() }
    }

    /// The layer of `id` (`block/oak_planks`) in `atlas`, added on first use, and how its alpha is used.
    pub fn layer(&mut self, id: &str, atlas: &mut Atlas) -> Option<(u16, Alpha)> {
        if let Some(known) = self.loaded.get(id) {
            return *known;
        }
        let added = self.add(id, atlas);
        if added.is_none() {
            self.missing.insert(id.to_owned());
        }
        self.loaded.insert(id.to_owned(), added);
        added
    }

    fn add(&self, id: &str, atlas: &mut Atlas) -> Option<(u16, Alpha)> {
        let file = self.assets.join(format!("textures/{id}.png"));
        let layer = u16::try_from(atlas.layers.len()).ok().filter(|l| usize::from(*l) < MAX_LAYERS)?;
        let meta = std::fs::read(self.assets.join(format!("textures/{id}.png.mcmeta"))).ok();
        let image = match meta.and_then(|m| flipbook(&serde_json::from_slice(&m).ok()?)) {
            Some(book) => {
                let strip = Texture::load_frames(&file, false).ok()?;
                let still = strip.first()?.clone();
                let animation = Animation::new(layer, strip, &book);
                let first = animation.as_ref().map_or(still, |a| a.at(0));
                atlas.animations.extend(animation);
                first
            }
            None => Texture::load(&file, false).ok()?,
        };
        let alpha = image.alpha();
        atlas.layers.push(image);
        Some((layer, alpha))
    }
}

/// How an `.mcmeta` animates its strip; `None` when it holds no animation.
// TODO: per-frame times are ignored; every frame takes `frametime`.
fn flipbook(meta: &Value) -> Option<Flipbook> {
    let animation = meta.get("animation")?;
    let index = |frame: &Value| frame.as_u64().or_else(|| frame.get("index")?.as_u64()).map(|i| i as usize);
    Some(Flipbook {
        frames: animation.get("frames").and_then(Value::as_array).map(|frames| frames.iter().filter_map(index).collect()),
        ticks_per_frame: animation.get("frametime").and_then(Value::as_u64).unwrap_or(1).max(1) as u32,
        blend: animation.get("interpolate").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn mcmeta_gives_frame_order_time_and_blending() {
        let book = flipbook(&json!({"animation": {"frametime": 3, "interpolate": true, "frames": [1, {"index": 0, "time": 9}]}})).unwrap();
        assert_eq!(book, Flipbook { frames: Some(vec![1, 0]), ticks_per_frame: 3, blend: true });
        assert_eq!(flipbook(&json!({"animation": {}})), Some(Flipbook { frames: None, ticks_per_frame: 1, blend: false }));
        assert_eq!(flipbook(&json!({"texture": {"blur": true}})), None);
    }
}
