//! A look's particle sprites in one image: Java's one file per sprite (`textures/particle/*.png`
//! from the client jar, tools/lookbake) packed into a grid, else the Bedrock pack's
//! `particles.png` sheet with `campfire_smoke.png` beside it, cut where Bedrock's particle files
//! (`particles/*.json`) put each sprite.

use std::path::Path;

use image::RgbaImage;

use crate::assets::image_file;

/// Whose particles a look draws: their sprites and how they move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    Java,
    #[default]
    Bedrock,
}

/// A sprite set: one sprite, or the frames a particle steps through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Set {
    /// Smallest to largest puff (`generic_0..7`).
    Generic,
    Flame,
    SoulFlame,
    Lava,
    Crit,
    EnchantedHit,
    Heart,
    Angry,
    Happy,
    Splash,
    Bubble,
    Note,
    DripHang,
    DripFall,
    DripLand,
    /// Campfire smoke.
    BigSmoke,
    Explosion,
}

const SETS: [Set; 17] = [
    Set::Generic, Set::Flame, Set::SoulFlame, Set::Lava, Set::Crit, Set::EnchantedHit, Set::Heart, Set::Angry, Set::Happy,
    Set::Splash, Set::Bubble, Set::Note, Set::DripHang, Set::DripFall, Set::DripLand, Set::BigSmoke, Set::Explosion,
];
/// Java sprites are packed into cells this many texels square, this many to a row.
const CELL: u32 = 32;
const COLUMNS: u32 = 16;
/// Where Bedrock's campfire smoke strip goes, right of the 128-texel sheet.
const STRIP_X: u32 = 128;

pub struct Sheet {
    pub style: Style,
    pub image: RgbaImage,
    /// Per [`SETS`] entry, each frame's UV rectangle `[u0, v0, u1, v1]`.
    frames: Vec<Vec<[f32; 4]>>,
}

impl Sheet {
    /// From a look's files; `None` when they hold neither layout.
    pub fn load(files: &Path) -> Option<Sheet> {
        java(files).or_else(|| bedrock(files))
    }

    /// Empty when the look lacks the set's sprites.
    pub fn frames(&self, set: Set) -> &[[f32; 4]] {
        &self.frames[set as usize]
    }

    fn new(style: Style, image: RgbaImage, texels: Vec<Vec<[u32; 4]>>) -> Sheet {
        let (w, h) = (image.width() as f32, image.height() as f32);
        let frames = texels.into_iter().map(|set| set.into_iter().map(|[x, y, sw, sh]| [x as f32 / w, y as f32 / h, (x + sw) as f32 / w, (y + sh) as f32 / h]).collect()).collect();
        Sheet { style, image, frames }
    }
}

fn java_names(set: Set) -> Vec<String> {
    let numbered = |name: &str, n: usize| (0..n).map(|i| format!("{name}_{i}")).collect();
    let one = |name: &str| vec![name.to_owned()];
    match set {
        Set::Generic => numbered("generic", 8),
        Set::Flame => one("flame"),
        Set::SoulFlame => one("soul_fire_flame"),
        Set::Lava => one("lava"),
        Set::Crit => one("critical_hit"),
        Set::EnchantedHit => one("enchanted_hit"),
        Set::Heart => one("heart"),
        Set::Angry => one("angry"),
        Set::Happy => one("glint"),
        Set::Splash => numbered("splash", 4),
        Set::Bubble => one("bubble"),
        Set::Note => one("note"),
        Set::DripHang => one("drip_hang"),
        Set::DripFall => one("drip_fall"),
        Set::DripLand => one("drip_land"),
        Set::BigSmoke => numbered("big_smoke", 12),
        Set::Explosion => numbered("explosion", 16),
    }
}

fn java(files: &Path) -> Option<Sheet> {
    let dir = files.join("textures/particle");
    image_file(&dir, "generic_0")?;
    let names: Vec<Vec<String>> = SETS.iter().map(|&s| java_names(s)).collect();
    let count = names.iter().map(Vec::len).sum::<usize>() as u32;
    let mut image = RgbaImage::new(CELL * COLUMNS, CELL * count.div_ceil(COLUMNS));
    let mut cell = 0;
    let mut texels = Vec::with_capacity(names.len());
    for set in names {
        let mut frames = Vec::new();
        for name in set {
            let Some(sprite) = image_file(&dir, &name).and_then(|p| image::open(p).ok()) else { continue };
            // An animated sprite is a vertical strip: its first frame.
            let side = sprite.width().min(sprite.height()).min(CELL);
            let sprite = sprite.crop_imm(0, 0, side, side).into_rgba8();
            let (x, y) = (cell % COLUMNS * CELL, cell / COLUMNS * CELL);
            image::imageops::replace(&mut image, &sprite, i64::from(x), i64::from(y));
            frames.push([x, y, side, side]);
            cell += 1;
        }
        texels.push(frames);
    }
    Some(Sheet::new(Style::Java, image, texels))
}

/// Texel rectangles on `particles.png` (x, y, w, h); [`STRIP_X`] onwards is the campfire strip.
fn bedrock_texels(set: Set) -> Vec<[u32; 4]> {
    let cell = |x, y| vec![[x, y, 8, 8]];
    let row = |y, n: u32| (0..n).map(|i| [i * 8, y, 8, 8]).collect();
    match set {
        Set::Generic => row(0, 8),
        Set::Flame => cell(0, 24),
        Set::SoulFlame => cell(16, 24),
        Set::Lava => vec![[10, 26, 4, 4]],
        Set::Crit => row(72, 8),
        Set::EnchantedHit => cell(16, 32),
        Set::Heart => cell(0, 40),
        Set::Angry => cell(8, 40),
        Set::Happy => cell(16, 40),
        Set::Splash => (0..4).map(|i| [24 + i * 8, 8, 8, 8]).collect(),
        Set::Bubble => cell(0, 16),
        Set::Note => cell(0, 32),
        Set::DripHang | Set::DripFall | Set::DripLand => cell(8, 56),
        Set::BigSmoke => (0..12).map(|i| [STRIP_X, i * 16, 16, 16]).collect(),
        Set::Explosion => row(80, 16),
    }
}

fn bedrock(files: &Path) -> Option<Sheet> {
    let open = |name: &str| Some(image::open(image_file(files, &format!("textures/particle/{name}"))?).ok()?.into_rgba8());
    let sheet = open("particles")?;
    let strip = open("campfire_smoke");
    let height = sheet.height().max(strip.as_ref().map_or(0, RgbaImage::height));
    let mut image = RgbaImage::new(STRIP_X + 16, height);
    image::imageops::replace(&mut image, &sheet, 0, 0);
    if let Some(strip) = &strip {
        image::imageops::replace(&mut image, strip, i64::from(STRIP_X), 0);
    }
    let texels = SETS.iter().map(|&s| if s == Set::BigSmoke && strip.is_none() { Vec::new() } else { bedrock_texels(s) }).collect();
    Some(Sheet::new(Style::Bedrock, image, texels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_index_their_own_slot() {
        assert!(SETS.iter().enumerate().all(|(i, &s)| s as usize == i));
    }

    #[test]
    fn bedrock_frames_land_on_the_sheet() {
        let image = RgbaImage::new(STRIP_X + 16, 192);
        let sheet = Sheet::new(Style::Bedrock, image, SETS.iter().map(|&s| bedrock_texels(s)).collect());
        assert_eq!(sheet.frames(Set::Explosion).len(), 16);
        assert!(sheet.frames.iter().flatten().all(|r| r.iter().all(|v| (0.0..=1.0).contains(v)) && r[0] < r[2] && r[1] < r[3]));
    }
}
