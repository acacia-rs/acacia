//! The vanilla resource pack subset fetched by `tools/fetch-vanilla-pack.sh`: `blocks.json` maps block
//! names to texture names per face, `textures/terrain_texture.json` maps texture names to image paths.

pub mod image;
pub mod json;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::Error;

/// Face order used everywhere in this crate: +X east, -X west, +Y up, -Y down, +Z south, -Z north.
pub const FACE_NAMES: [&str; 6] = ["east", "west", "up", "down", "south", "north"];

pub struct Pack {
    root: PathBuf,
    blocks: Map<String, Value>,
    terrain: Map<String, Value>,
}

/// One resolved texture: the image path without extension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureRef {
    pub path: String,
    /// Alpha marks a tint overlay.
    pub overlay: bool,
    /// Each frame holds 2×2 copies of the tile (flowing water and lava).
    pub quad: bool,
}

impl Pack {
    /// `$ACACIA_ASSETS`, else `assets/vanilla` under the working directory.
    pub fn default_dir() -> PathBuf {
        std::env::var_os("ACACIA_ASSETS").map_or_else(|| PathBuf::from("assets/vanilla"), PathBuf::from)
    }

    pub fn load(root: &Path) -> Result<Pack, Error> {
        let object = |v: Value| match v {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let blocks = object(json::read(&root.join("blocks.json"))?);
        let mut terrain = object(json::read(&root.join("textures/terrain_texture.json"))?);
        let terrain = object(terrain.remove("texture_data").unwrap_or_default());
        Ok(Pack { root: root.to_owned(), blocks, terrain })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Texture names per face (see [`FACE_NAMES`]) for a block name without namespace.
    pub fn block_faces(&self, name: &str) -> Option<[String; 6]> {
        let textures = self.blocks.get(name)?.get("textures")?;
        match textures {
            Value::String(s) => Some(std::array::from_fn(|_| s.clone())),
            Value::Object(m) => {
                let get = |k: &str| m.get(k).and_then(Value::as_str);
                let any = get("side").or(get("up")).or_else(|| m.values().find_map(Value::as_str))?;
                Some(FACE_NAMES.map(|face| {
                    let horizontal = !matches!(face, "up" | "down");
                    get(face).or(if horizontal { get("side") } else { None }).unwrap_or(any).to_owned()
                }))
            }
            _ => None,
        }
    }

    /// First variant of a terrain texture. Arrays hold legacy data-value variants or random
    /// variations; leaves list `[fancy, opaque]`.
    pub fn texture(&self, name: &str) -> Option<TextureRef> {
        let entry = self.terrain.get(name)?;
        let quad = entry.get("quad").and_then(Value::as_i64) == Some(1);
        let first = match entry.get("textures")? {
            Value::Array(a) => a.first()?,
            v => v,
        };
        match first {
            Value::String(s) => Some(TextureRef { path: s.clone(), overlay: false, quad }),
            Value::Object(o) => {
                let path = o.get("path").and_then(Value::as_str).or_else(|| {
                    o.get("variations")?.as_array()?.first()?.get("path")?.as_str()
                })?;
                Some(TextureRef { path: path.to_owned(), overlay: o.contains_key("overlay_color"), quad })
            }
            _ => None,
        }
    }

    /// The image file for a texture path, trying `.png` then `.tga`.
    pub fn image_file(&self, path: &str) -> Option<PathBuf> {
        ["png", "tga"].iter().map(|ext| self.root.join(format!("{path}.{ext}"))).find(|p| p.is_file())
    }
}
