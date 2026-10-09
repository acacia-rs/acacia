//! A theme is the atlas of HUD sprites under their canonical names ([`crate::hud::sprite`]), the
//! font, and the few layout choices that differ between the games. Loaders: `java.rs` reads the
//! jar's unpacked assets, `bedrock.rs` the Bedrock resource pack. Sources: README.md "Themes".

pub mod bedrock;
pub mod java;

use std::path::Path;

use image::RgbaImage;

use crate::atlas::Atlas;
use crate::font::Font;
use crate::widget::Widgets;

/// Layout choices the games make differently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Colour of the experience level number.
    pub xp_colour: u32,
    /// Java outlines the level in black; Bedrock gives it a drop shadow.
    pub xp_outline: bool,
    /// Sign text takes Java's dye colours, at 40% unless it glows; Bedrock draws the stored colour.
    pub java_signs: bool,
}

pub struct Theme {
    pub atlas: Atlas,
    /// `None` when no font was found: text is not drawn.
    pub font: Option<Font>,
    pub style: Style,
    pub widgets: Widgets,
}

pub(crate) fn png(path: &Path) -> Option<RgbaImage> {
    match image::open(path) {
        Ok(image) => Some(image.to_rgba8()),
        Err(e) => {
            tracing::debug!(path = %path.display(), %e, "ui image");
            None
        }
    }
}

/// Java's ASCII page under `root` (unpacked jar layout), with the characters its font definition
/// gives each cell.
pub(crate) fn java_font(root: &Path, atlas: &mut Atlas) -> Option<Font> {
    let image = png(&root.join("textures/font/ascii.png"))?;
    let rows = ascii_rows(root).unwrap_or_else(|| Font::code_page_rows());
    let page = atlas.add("font/ascii", &image);
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    Some(Font::from_grid(&image, page, &rows))
}

/// The `chars` of the bitmap provider drawing `ascii.png` in `font/include/default.json`.
fn ascii_rows(root: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(root.join("font/include/default.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let provider = json["providers"].as_array()?.iter().find(|p| p["file"].as_str().is_some_and(|f| f.ends_with("font/ascii.png")))?;
    provider["chars"].as_array()?.iter().map(|row| row.as_str().map(str::to_owned)).collect()
}
