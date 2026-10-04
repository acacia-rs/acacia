//! A look pack on disk: `pack.json` (version, look, blocks, states, animations), `textures.png` (the
//! array layers stacked top to bottom) and `frames.png` (every animation's frames, in `pack.json`'s order).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::LookPack;
use crate::Error;
use crate::assets::flipbook::{Animation, Atlas};
use crate::assets::image::{TEXEL_BYTES, TEXTURE_SIZE, Texture};
use crate::blocks::RenderBlock;
use crate::entity::EntityModels;
use crate::look::Look;

/// Packs written as another version are refused; bake again.
pub const VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
struct PackFile {
    version: u64,
    look: Look,
    blocks: Vec<RenderBlock>,
    states: BTreeMap<String, u32>,
    animations: Vec<AnimationEntry>,
}

#[derive(Serialize, Deserialize)]
struct AnimationEntry {
    layer: u16,
    frames: usize,
    ticks_per_frame: u32,
    blend: bool,
}

impl LookPack {
    pub fn save(&self, dir: &Path) -> Result<(), Error> {
        let io = |path: &Path| { let path = path.display().to_string(); move |source| Error::Io { path, source } };
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        let animations = &self.atlas.animations;
        let file = PackFile {
            version: VERSION,
            look: self.look,
            blocks: self.blocks.clone(),
            states: self.states.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            animations: animations.iter().map(|a| AnimationEntry { layer: a.layer, frames: a.frames.len(), ticks_per_frame: a.ticks_per_frame, blend: a.blend }).collect(),
        };
        let json = dir.join("pack.json");
        std::fs::write(&json, serde_json::to_vec(&file).expect("a pack serializes")).map_err(io(&json))?;
        write_strip(&dir.join("textures.png"), self.atlas.layers.iter())?;
        write_strip(&dir.join("frames.png"), animations.iter().flat_map(|a| &a.frames))?;
        if self.files == dir {
            return Ok(());
        }
        for name in LOOSE {
            let from = self.files.join(name);
            copy_tree(&from, &dir.join(name)).map_err(io(&from))?;
        }
        // Entity definitions also name images outside textures/entity (pottery patterns).
        for image in EntityModels::load(&self.files).textures() {
            if let Ok(relative) = image.strip_prefix(&self.files) {
                copy_tree(image, &dir.join(relative)).map_err(io(image))?;
            }
        }
        Ok(())
    }

    pub fn load(dir: &Path) -> Result<LookPack, Error> {
        let json = dir.join("pack.json");
        let path = json.display().to_string();
        let bad = |reason: String| Error::LookPack { path: path.clone(), reason };
        let text = std::fs::read(&json).map_err(|source| Error::Io { path: path.clone(), source })?;
        let value: serde_json::Value = serde_json::from_slice(&text).map_err(|source| Error::Json { path: path.clone(), source })?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version != Some(VERSION) {
            return Err(bad(format!("look pack version {version:?}, this build reads {VERSION}")));
        }
        let file: PackFile = serde_json::from_value(value).map_err(|source| Error::Json { path: path.clone(), source })?;
        if let Some(index) = file.states.values().find(|&&i| i as usize >= file.blocks.len()) {
            return Err(bad(format!("state points at block {index} of {}", file.blocks.len())));
        }
        let layers = read_strip(&dir.join("textures.png"))?;
        let mut frames = if file.animations.is_empty() { Vec::new() } else { read_strip(&dir.join("frames.png"))? }.into_iter();
        let mut animations = Vec::new();
        for a in &file.animations {
            let frames: Vec<Texture> = frames.by_ref().take(a.frames).collect();
            if frames.len() != a.frames || frames.is_empty() {
                return Err(bad(format!("frames.png is short for layer {}", a.layer)));
            }
            animations.push(Animation { layer: a.layer, frames, ticks_per_frame: a.ticks_per_frame.max(1), blend: a.blend });
        }
        Ok(LookPack { look: file.look, blocks: file.blocks, states: file.states.into_iter().collect(), atlas: Atlas { layers, animations }, files: dir.to_owned() })
    }
}

/// What [`LookPack::files`] holds, copied beside the baked data.
const LOOSE: [&str; 9] = [
    "entity", "models", "render_controllers", "animations", "animation_controllers",
    "textures/entity", "textures/environment", "textures/colormap", "biomes_client.json",
];

/// Copies a file or a directory's files; a source that does not exist is skipped.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else if from.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to)?;
    }
    Ok(())
}

fn image_error(path: &Path) -> impl Fn(image::ImageError) -> Error {
    let path = path.display().to_string();
    move |source| Error::Image { path: path.clone(), source }
}

/// Nothing is written for no textures: an image cannot be empty.
fn write_strip<'a>(path: &Path, textures: impl Iterator<Item = &'a Texture>) -> Result<(), Error> {
    let bytes: Vec<u8> = textures.flat_map(|t| t.rgba.iter().copied()).collect();
    if bytes.is_empty() {
        return Ok(());
    }
    let height = (bytes.len() / TEXEL_BYTES) as u32 * TEXTURE_SIZE;
    image::save_buffer(path, &bytes, TEXTURE_SIZE, height, image::ColorType::Rgba8).map_err(image_error(path))
}

fn read_strip(path: &Path) -> Result<Vec<Texture>, Error> {
    let image = image::open(path).map_err(image_error(path))?.into_rgba8();
    if image.width() != TEXTURE_SIZE || image.height() % TEXTURE_SIZE != 0 {
        let reason = format!("{}x{} is not a strip of {TEXTURE_SIZE}x{TEXTURE_SIZE} textures", image.width(), image.height());
        return Err(Error::LookPack { path: path.display().to_string(), reason });
    }
    let (textures, _) = image.as_raw().as_chunks::<TEXEL_BYTES>();
    Ok(textures.iter().map(|rgba| Texture { rgba: Box::new(*rgba) }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BlockTable;

    fn grey(v: u8) -> Texture {
        Texture { rgba: Box::new([v; TEXEL_BYTES]) }
    }

    fn sample(name: &str) -> LookPack {
        let animation = Animation { layer: 1, frames: vec![grey(10), grey(20)], ticks_per_frame: 3, blend: false };
        LookPack {
            look: Look::BEDROCK,
            blocks: vec![BlockTable::cube(0), BlockTable::cube(1)],
            states: [("minecraft:air".to_owned(), 0), ("minecraft:oak_log[pillar_axis=y]".to_owned(), 1)].into_iter().collect(),
            atlas: Atlas { layers: vec![Texture::missing(), grey(10)], animations: vec![animation] },
            files: temp(&format!("{name}-loose")),
        }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("acacia-lookpack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_saved_pack_loads_back_the_same() {
        let (dir, pack) = (temp("same"), sample("same"));
        std::fs::create_dir_all(pack.files.join("textures/colormap")).unwrap();
        std::fs::write(pack.files.join("textures/colormap/grass.png"), b"loose").unwrap();
        pack.save(&dir).unwrap();
        let loaded = LookPack::load(&dir).unwrap();
        assert_eq!(std::fs::read(loaded.files().join("textures/colormap/grass.png")).unwrap(), b"loose");
        std::fs::remove_dir_all(&pack.files).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!((loaded.look, &loaded.blocks, &loaded.states), (pack.look, &pack.blocks, &pack.states));
        let texels = |atlas: &Atlas| atlas.layers.iter().map(|t| t.rgba.to_vec()).collect::<Vec<_>>();
        assert_eq!(texels(&loaded.atlas), texels(&pack.atlas));
        let animation = &loaded.atlas.animations[0];
        assert_eq!((animation.layer, animation.at(0).rgba[0], animation.at(3).rgba[0]), (1, 10, 20));
    }

    #[test]
    fn another_version_is_refused() {
        let dir = temp("version");
        sample("version").save(&dir).unwrap();
        let json = dir.join("pack.json");
        let text = std::fs::read_to_string(&json).unwrap().replacen("\"version\":1", "\"version\":0", 1);
        std::fs::write(&json, text).unwrap();
        let error = LookPack::load(&dir).err().expect("refused").to_string();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(error.contains("version"), "{error}");
    }
}
