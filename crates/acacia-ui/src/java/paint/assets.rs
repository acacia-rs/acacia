//! A Java resource pack's GUI images for the painter, packed into the UI atlas on first use:
//! `textures/gui/sprites/<id>.png` with its `.mcmeta` scaling, whole textures, block sprites.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::atlas::{Atlas, Sprite};
use crate::theme::png;

use super::blit::Scaling;

pub struct Images {
    /// The pack's `assets/minecraft`.
    root: PathBuf,
    /// The GUI atlas's directory sources (`atlases/gui.json`): sprite id prefix and folder.
    sources: Vec<(String, String)>,
    scaling: HashMap<String, Option<Scaling>>,
}

/// `atlases/gui.json`'s `directory` sources; `gui/sprites` alone when it can't be read.
fn gui_sources(root: &Path) -> Vec<(String, String)> {
    let json: serde_json::Value = std::fs::read_to_string(root.join("atlases/gui.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let sources: Vec<(String, String)> = json["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["type"].as_str().is_some_and(|t| t.ends_with("directory")))
        .filter_map(|s| Some((s["prefix"].as_str().unwrap_or("").to_owned(), s["source"].as_str()?.to_owned())))
        .collect();
    if sources.is_empty() { vec![(String::new(), "gui/sprites".into())] } else { sources }
}

/// `minecraft:hud/heart/full` → `hud/heart/full`.
fn path(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

impl Images {
    pub fn new(root: &Path) -> Images {
        Images { root: root.to_owned(), sources: gui_sources(root), scaling: HashMap::new() }
    }

    /// The GUI sprite `id`'s file without `.png`: the last source holding it wins, as later
    /// sources override earlier ones (`mob_effect/speed` is `textures/mob_effect/speed`).
    pub fn sprite_file(&self, id: &str) -> Option<PathBuf> {
        let id = path(id);
        self.sources.iter().rev().find_map(|(prefix, source)| {
            let base = self.root.join("textures").join(source).join(id.strip_prefix(prefix.as_str())?);
            base.with_extension("png").is_file().then_some(base)
        })
    }

    pub fn sprite(&mut self, atlas: &mut Atlas, id: &str) -> Option<(Sprite, Scaling)> {
        let id = path(id);
        let name = format!("gui/{id}");
        if let Some(cached) = self.scaling.get(id) {
            return Some((atlas.get(&name)?, (*cached)?));
        }
        let Some(base) = self.sprite_file(id) else {
            let missing = (id == MISSING).then(|| (atlas.add(&name, &missing_image()), Scaling::Stretch));
            self.scaling.insert(id.to_owned(), missing.map(|m| m.1));
            return missing;
        };
        let meta: serde_json::Value = std::fs::read_to_string(base.with_extension("png.mcmeta")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let image = png(&base.with_extension("png")).map(|i| if meta.get("animation").is_some() { first_frame(i) } else { i });
        let found = image.map(|i| (atlas.add(&name, &i), Scaling::from_mcmeta(&meta)));
        self.scaling.insert(id.to_owned(), found.map(|f| f.1));
        found
    }

    /// `path` under `textures/` without `.png`.
    pub fn texture(&mut self, atlas: &mut Atlas, path_: &str) -> Option<Sprite> {
        let path_ = path(path_);
        let name = format!("tex/{path_}");
        if let Some(s) = atlas.get(&name) {
            return Some(s);
        }
        let image = png(&self.root.join("textures").join(format!("{path_}.png")))?;
        Some(atlas.add(&name, &image))
    }

    /// A block atlas sprite (`block/nether_portal`): its first animation frame.
    pub fn block_sprite(&mut self, atlas: &mut Atlas, id: &str) -> Option<Sprite> {
        let id = path(id);
        let name = format!("blocksprite/{id}");
        if let Some(s) = atlas.get(&name) {
            return Some(s);
        }
        let image = png(&self.root.join("textures").join(format!("{id}.png")))?;
        Some(atlas.add(&name, &first_frame(image)))
    }
}

/// The atlas's missing sprite, which stands in for any sprite it lacks.
pub const MISSING: &str = "missingno";

/// `MissingTextureAtlasSprite.generateMissingImage`: 8 × 8 black and magenta squares.
fn missing_image() -> image::RgbaImage {
    image::RgbaImage::from_fn(16, 16, |x, y| if (x < 8) ^ (y < 8) { image::Rgba([248, 0, 248, 255]) } else { image::Rgba([0, 0, 0, 255]) })
}

/// An animated sprite's strip cut to its first square frame.
fn first_frame(image: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = image.dimensions();
    if h > w && h % w == 0 { image::imageops::crop_imm(&image, 0, 0, w, w).to_image() } else { image }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_icons_are_gui_sprites_from_their_own_folder() {
        let root = std::env::temp_dir().join(format!("acacia-gui-sources-{}", std::process::id()));
        let file = |p: &str, text: &str| {
            let p = root.join(p);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let sources = r#"{"sources": [{"type": "minecraft:directory", "prefix": "", "source": "gui/sprites"},
            {"type": "minecraft:directory", "prefix": "mob_effect/", "source": "mob_effect"}]}"#;
        file("atlases/gui.json", sources);
        file("textures/mob_effect/speed.png", "");
        file("textures/gui/sprites/hud/hotbar.png", "");
        let images = Images::new(&root);
        assert_eq!(images.sprite_file("minecraft:mob_effect/speed"), Some(root.join("textures/mob_effect/speed")));
        assert_eq!(images.sprite_file("hud/hotbar"), Some(root.join("textures/gui/sprites/hud/hotbar")));
        assert_eq!(images.sprite_file("mob_effect/haste"), None);
        std::fs::remove_dir_all(&root).ok();
    }
}
