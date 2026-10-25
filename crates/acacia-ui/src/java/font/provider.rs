//! Glyph providers as `GlyphProviderDefinition`s load them: `bitmap` (a sheet of cells, each as
//! wide as its rightmost lit column), `space` (advances only) and `unihex`. Vanilla 26.3 uses no
//! `ttf` provider (none of its font definitions names one), so a `ttf` definition fails to load as
//! a provider whose file is missing would.

use std::collections::HashMap;
use std::sync::Arc;

use image::DynamicImage;
use serde_json::Value;

use super::hashset::{IntSet, ascending};
use super::resources::{Resources, split_id};
use super::unihex::{self, HexGlyph, Override};

/// Where a glyph's texels are.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GlyphTexture {
    /// A bitmap provider's sheet, by resource id (`minecraft:textures/font/ascii.png`).
    Sheet(Arc<str>),
    /// One Unifont glyph (`width` × 16 texels, `Fonts::texels`): the zip's id and the codepoint.
    Unihex(Arc<str>, u32),
    /// `SpecialGlyphs.MISSING`: a 5 × 8 box outline.
    Missing,
    /// `SpecialGlyphs.WHITE`: 5 × 8 white, what effects and backgrounds draw with.
    White,
}

/// A glyph's `GlyphBitmap` as `FontTexture.add` bakes it: texels `src` (x, y, w, h) of `texture`
/// and the box it covers relative to the pen (left, right, up, down in GUI pixels).
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub texture: GlyphTexture,
    pub src: [u32; 4],
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
    /// More than one channel: `GUI_TEXT`; one: `GUI_TEXT_GRAYSCALE`.
    pub colored: bool,
}

/// A glyph: `GlyphInfo` and what it draws (`None` for a space).
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub advance: f32,
    pub bold_offset: f32,
    pub shadow_offset: f32,
    pub bitmap: Option<Bitmap>,
}

impl Glyph {
    /// `getAdvance(bold)`.
    pub fn advance(&self, bold: bool) -> f32 {
        self.advance + if bold { self.bold_offset } else { 0.0 }
    }
}

/// `GlyphBitmap`'s box from its texel size, oversample and bearing top.
fn bitmap(texture: GlyphTexture, src: [u32; 4], oversample: f32, bearing_top: f32, colored: bool) -> Bitmap {
    let top = 7.0 - bearing_top;
    Bitmap { texture, src, left: 0.0, right: src[2] as f32 / oversample, up: top, down: top + src[3] as f32 / oversample, colored }
}

/// `SpecialGlyphs`: advance 6, 5 × 8 texels at the baseline's top.
pub fn special(texture: GlyphTexture) -> Glyph {
    Glyph { advance: 6.0, bold_offset: 1.0, shadow_offset: 1.0, bitmap: Some(bitmap(texture, [0, 0, 5, 8], 1.0, 7.0, true)) }
}

pub struct Sheet {
    pub id: Arc<str>,
    pub image: Arc<DynamicImage>,
    /// Cell size and `height / cellHeight`.
    cell: [u32; 2],
    scale: f32,
    ascent: i32,
    /// Codepoint to (column, row, advance).
    glyphs: HashMap<u32, (u32, u32, i32)>,
}

pub enum Provider {
    Space(HashMap<u32, f32>, IntSet),
    Bitmap(Sheet, IntSet),
    Unihex(Arc<str>, HashMap<u32, HexGlyph>, IntSet),
}

impl Provider {
    pub fn glyph(&self, c: u32) -> Option<Glyph> {
        match self {
            Provider::Space(advances, _) => advances.get(&c).map(|&advance| Glyph { advance, bold_offset: 1.0, shadow_offset: 1.0, bitmap: None }),
            Provider::Bitmap(s, _) => s.glyphs.get(&c).map(|&(col, row, advance)| {
                let src = [col * s.cell[0], row * s.cell[1], s.cell[0], s.cell[1]];
                let b = bitmap(GlyphTexture::Sheet(s.id.clone()), src, 1.0 / s.scale, s.ascent as f32, s.image.color().channel_count() > 1);
                Glyph { advance: advance as f32, bold_offset: 1.0, shadow_offset: 1.0, bitmap: Some(b) }
            }),
            Provider::Unihex(file, glyphs, _) => glyphs.get(&c).map(|g| Glyph {
                advance: g.advance(),
                bold_offset: 0.5,
                shadow_offset: 0.5,
                bitmap: Some(bitmap(GlyphTexture::Unihex(file.clone(), c), [0, 0, g.width().max(0) as u32, 16], 2.0, 7.0, true)),
            }),
        }
    }

    /// `getSupportedGlyphs()`, in its iteration order.
    pub fn supported(&self) -> &IntSet {
        match self {
            Provider::Space(_, s) | Provider::Bitmap(_, s) | Provider::Unihex(_, _, s) => s,
        }
    }
}

/// `ExtraCodecs.CODEPOINT`: a string of exactly one codepoint.
fn codepoint(v: &Value) -> Option<u32> {
    let s = v.as_str()?;
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(u32::from(c))
}

/// `Definition.load` of a non-reference provider definition.
pub fn load(def: &Value, resources: &Resources) -> Result<Provider, String> {
    match def["type"].as_str() {
        Some("space") => {
            let advances = def["advances"].as_object().ok_or("no advances")?;
            let mut map = HashMap::new();
            let mut keys = IntSet::with_expected(advances.len());
            for (k, v) in advances {
                let c = codepoint(&Value::String(k.clone())).ok_or("advance key is not one codepoint")?;
                map.insert(c, v.as_f64().ok_or("advance is not a number")? as f32);
                keys.add(c as i32);
            }
            Ok(Provider::Space(map, keys))
        }
        Some("bitmap") => load_bitmap(def, resources),
        Some("unihex") => {
            let file = def["hex_file"].as_str().ok_or("no hex_file")?;
            let mut overrides = Vec::new();
            for o in def["size_overrides"].as_array().into_iter().flatten() {
                let (from, to) = (codepoint(&o["from"]).ok_or("bad from")?, codepoint(&o["to"]).ok_or("bad to")?);
                if from >= to {
                    return Err(format!("invalid range [{from};{to}]"));
                }
                let int = |k: &str| o[k].as_i64().map(|v| v as i32).ok_or(format!("no {k}"));
                overrides.push(Override { from, to, left: int("left")?, right: int("right")? });
            }
            let id = qualify(file);
            let zip = resources.read(&id).ok_or_else(|| format!("{id} not found"))?;
            let glyphs = unihex::load(&zip, &overrides)?;
            let mut codepoints: Vec<u32> = glyphs.keys().copied().collect();
            codepoints.sort_unstable();
            Ok(Provider::Unihex(id.into(), glyphs, ascending(codepoints)))
        }
        other => Err(format!("unsupported provider type {other:?}")),
    }
}

/// An id with its namespace (`Identifier.parse`).
pub fn qualify(id: &str) -> String {
    let (ns, path) = split_id(id);
    format!("{ns}:{path}")
}

fn load_bitmap(def: &Value, resources: &Resources) -> Result<Provider, String> {
    let file = def["file"].as_str().ok_or("no file")?;
    let height = def.get("height").map_or(Some(8), Value::as_i64).ok_or("bad height")? as i32;
    let ascent = def["ascent"].as_i64().ok_or("no ascent")? as i32;
    let grid: Vec<Vec<u32>> = def["chars"].as_array().ok_or("no chars")?.iter().map(|r| r.as_str().map(|s| s.chars().map(u32::from).collect())).collect::<Option<_>>().ok_or("chars are not strings")?;
    if grid.is_empty() || grid[0].is_empty() || grid.iter().any(|r| r.len() != grid[0].len()) {
        return Err("bad codepoint grid".into());
    }
    if ascent > height {
        return Err(format!("ascent {ascent} higher than height {height}"));
    }
    let (ns, path) = split_id(file);
    let id = format!("{ns}:textures/{path}");
    let bytes = resources.read(&id).ok_or_else(|| format!("{id} not found"))?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|e| format!("{id}: {e}"))?;
    let lit = lit_channel(&image).ok_or_else(|| format!("{id}: no luminance or alpha"))?;
    let (w, h) = (image.width(), image.height());
    let cell = [w / grid[0].len() as u32, h / grid.len() as u32];
    let scale = height as f32 / cell[1] as f32;
    let mut glyphs = HashMap::new();
    let mut codepoints = Vec::new();
    for (row, line) in grid.iter().enumerate() {
        for (col, &c) in line.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let (col, row) = (col as u32, row as u32);
            let width = (0..cell[0]).rev().find(|&x| (0..cell[1]).any(|y| lit(col * cell[0] + x, row * cell[1] + y) != 0)).map_or(0, |x| x + 1);
            if glyphs.insert(c, (col, row, (0.5 + f64::from(width as f32 * scale)) as i32 + 1)).is_none() {
                codepoints.push(c);
            }
        }
    }
    codepoints.sort_unstable();
    drop(lit);
    let sheet = Sheet { id: id.into(), image: Arc::new(image), cell, scale, ascent, glyphs };
    Ok(Provider::Bitmap(sheet, ascending(codepoints)))
}

/// `NativeImage.getLuminanceOrAlpha`: luminance when the image has it, else alpha; RGB has neither.
fn lit_channel(image: &DynamicImage) -> Option<Box<dyn Fn(u32, u32) -> u8 + '_>> {
    Some(match image {
        DynamicImage::ImageLuma8(i) => Box::new(|x, y| i.get_pixel(x, y).0[0]),
        DynamicImage::ImageLumaA8(i) => Box::new(|x, y| i.get_pixel(x, y).0[0]),
        DynamicImage::ImageRgba8(i) => Box::new(|x, y| i.get_pixel(x, y).0[3]),
        DynamicImage::ImageLuma16(i) => Box::new(|x, y| (i.get_pixel(x, y).0[0] >> 8) as u8),
        DynamicImage::ImageLumaA16(i) => Box::new(|x, y| (i.get_pixel(x, y).0[0] >> 8) as u8),
        DynamicImage::ImageRgba16(i) => Box::new(|x, y| (i.get_pixel(x, y).0[3] >> 8) as u8),
        _ => return None,
    })
}
