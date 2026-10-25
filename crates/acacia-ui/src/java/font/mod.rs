//! Java 26.3's fonts as `FontManager` loads them (every font definition, its providers, the font
//! options' filters, the missing glyph), measured as `Font.width` measures (`Metrics`) and laid out
//! as `Font.prepareText` lays out (`layout`). Unifont is not in the client jar: it comes from the
//! version's asset index (`Resources::java`).

pub mod hashset;
pub mod layout;
pub mod manager;
pub mod provider;
pub mod resources;
pub mod set;
pub mod unihex;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

pub use layout::{GlyphQuad, Pen, Prepared};
pub use provider::{Bitmap, Glyph, GlyphTexture, Provider};
pub use resources::Resources;
pub use set::{FontOptions, FontSet};

use super::gui::Metrics;
use super::text::{Style, Text};
use set::Conditional;

/// `FontDescription.DEFAULT`.
pub const DEFAULT: &str = "minecraft:default";
/// 26.3's `assetIndex.id`.
pub const ASSET_INDEX: &str = "34";

pub struct Fonts {
    defs: BTreeMap<String, Vec<Conditional>>,
    sets: HashMap<String, FontSet>,
    /// `FontManager.missingFontSet`, for styles naming a font that is not loaded.
    missing: FontSet,
    files: HashMap<Arc<str>, Arc<Provider>>,
    options: FontOptions,
    /// What loading warned about.
    pub warnings: Vec<String>,
}

/// A character as the font sees it after `Language.getVisualOrder`: its style, and whether it
/// starts a run of one style (position 0, where underlines start a pixel early).
#[derive(Clone, Debug, PartialEq)]
pub struct VisualChar {
    pub first: bool,
    pub style: Style,
    pub codepoint: u32,
}

impl Fonts {
    pub fn load(resources: &Resources, options: FontOptions) -> Fonts {
        let mut warnings = Vec::new();
        let defs = manager::load(resources, &mut warnings);
        let files = manager::by_file(&defs);
        let sets = defs.iter().map(|(id, list)| (id.clone(), FontSet::new(list.clone(), options))).collect();
        Fonts { defs, sets, missing: FontSet::new(Vec::new(), options), files, options, warnings }
    }

    /// The game's fonts: the jar's `assets` (its `assets/minecraft`) under the asset index at
    /// `index_root` (`indexes/` and `objects/`), when it is there.
    pub fn java(assets: &Path, index_root: Option<&Path>, options: FontOptions) -> Fonts {
        Fonts::load(&Resources::java(assets, index_root.map(|r| (r, ASSET_INDEX))), options)
    }

    /// `FontManager.updateOptions`.
    pub fn set_options(&mut self, options: FontOptions) {
        self.options = options;
        for set in self.sets.values_mut() {
            set.reload(options);
        }
    }

    pub fn options(&self) -> FontOptions {
        self.options
    }

    /// The loaded fonts' ids.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.defs.keys().map(String::as_str)
    }

    /// `getFontSetRaw` for a style's font (`None` is the default font).
    pub fn set(&self, font: Option<&str>) -> &FontSet {
        self.sets.get(font.unwrap_or(DEFAULT)).unwrap_or(&self.missing)
    }

    /// `Font.getGlyphSource(style.font).getGlyph(c)` (`Minecraft.font`: fishy advances kept).
    pub fn glyph(&self, c: u32, style: &Style) -> Glyph {
        self.set(style.font.as_deref()).glyph(c, false)
    }

    /// A bitmap provider's sheet by its id (`minecraft:textures/font/ascii.png`).
    pub fn sheet(&self, id: &str) -> Option<&image::DynamicImage> {
        match self.files.get(id).map(|p| &**p) {
            Some(Provider::Bitmap(sheet, _)) => Some(&sheet.image),
            _ => None,
        }
    }

    /// A texture's texels as RGBA (width, height, pixels): a whole sheet, one Unifont glyph or a
    /// special glyph.
    pub fn texels(&self, texture: &GlyphTexture) -> Option<(u32, u32, Vec<u8>)> {
        match texture {
            GlyphTexture::Sheet(id) => self.sheet(id).map(|i| (i.width(), i.height(), i.to_rgba8().into_raw())),
            GlyphTexture::Unihex(file, c) => match self.files.get(file).map(|p| &**p) {
                Some(Provider::Unihex(_, glyphs, _)) => glyphs.get(c).map(|g| (g.width().max(0) as u32, 16, g.rgba())),
                _ => None,
            },
            GlyphTexture::Missing => Some((5, 8, (0..40).flat_map(|i| if matches!(i % 5, 0 | 4) || i < 5 || i >= 35 { [255; 4] } else { [0; 4] }).collect())),
            GlyphTexture::White => Some((5, 8, vec![255; 160])),
        }
    }

    /// The characters `Language.getVisualOrder` feeds the font for left-to-right text: legacy
    /// codes resolved, each run of one style starting at position 0.
    // TODO: right-to-left runs (ICU Bidi, Arabic shaping and mirroring) as ClientLanguage reorders them.
    pub fn visual_order(&self, text: &Text) -> Vec<VisualChar> {
        let mut out: Vec<VisualChar> = Vec::new();
        for (c, style) in text.resolve_legacy().chars() {
            let first = out.last().is_none_or(|last| last.style != *style);
            out.push(VisualChar { first, style: style.clone(), codepoint: u32::from(c) });
        }
        out
    }
}

impl Metrics for Fonts {
    fn advance(&self, c: char, style: &Style) -> f32 {
        self.glyph(u32::from(c), style).advance(style.is_bold())
    }
}

#[cfg(test)]
mod tests;
