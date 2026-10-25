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
    scaling: HashMap<String, Option<Scaling>>,
}

/// `minecraft:hud/heart/full` → `hud/heart/full`.
fn path(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

impl Images {
    pub fn new(root: &Path) -> Images {
        Images { root: root.to_owned(), scaling: HashMap::new() }
    }

    pub fn sprite(&mut self, atlas: &mut Atlas, id: &str) -> Option<(Sprite, Scaling)> {
        let id = path(id);
        let name = format!("gui/{id}");
        let base = self.root.join("textures/gui/sprites").join(id);
        if let Some(cached) = self.scaling.get(id) {
            return Some((atlas.get(&name)?, (*cached)?));
        }
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

/// An animated sprite's strip cut to its first square frame.
fn first_frame(image: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = image.dimensions();
    if h > w && h % w == 0 { image::imageops::crop_imm(&image, 0, 0, w, w).to_image() } else { image }
}
