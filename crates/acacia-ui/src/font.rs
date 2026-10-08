//! Bitmap fonts: a page of 16×16 glyph cells (Java's `ascii.png`, Bedrock's `default8.png`), each
//! glyph as wide as its rightmost opaque column, with `§` colour codes and a drop shadow.

use std::collections::HashMap;

use image::RgbaImage;

use crate::atlas::Sprite;
use crate::draw::DrawList;

/// GUI pixels between lines (Java's `Font.lineHeight`).
pub const LINE_HEIGHT: f32 = 9.0;
/// A glyph cell is this many GUI pixels tall whatever the page's resolution.
const GLYPH_HEIGHT: f32 = 8.0;
const SPACE_ADVANCE: f32 = 4.0;

/// The 16 colours of `§0` to `§f`.
pub const COLOURS: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA,
    0x555555, 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

#[derive(Debug, Clone, Copy)]
struct Glyph {
    /// Within the page, in texels.
    cell: [u32; 2],
    /// Ink width in GUI pixels.
    width: f32,
}

pub struct Font {
    page: Sprite,
    /// Texels per cell side.
    cell: u32,
    glyphs: HashMap<char, Glyph>,
}

impl Font {
    /// A page of 16×16 cells. `rows` names the characters in each row, left to right (Java's
    /// `chars`); `\0` marks an empty cell. `page` is where `image` went in the atlas.
    pub fn from_grid(image: &RgbaImage, page: Sprite, rows: &[&str]) -> Font {
        let cell = image.width() / 16;
        let mut glyphs = HashMap::new();
        for (row, chars) in rows.iter().enumerate() {
            for (col, c) in chars.chars().enumerate().filter(|&(_, c)| c != '\0') {
                let (x0, y0) = (col as u32 * cell, row as u32 * cell);
                let ink = (0..cell).rev().find(|&x| (0..cell).any(|y| image.get_pixel(x0 + x, y0 + y).0[3] > 0)).map_or(0, |x| x + 1);
                let width = ink as f32 * GLYPH_HEIGHT / cell as f32;
                glyphs.insert(c, Glyph { cell: [x0, y0], width });
            }
        }
        Font { page, cell, glyphs }
    }

    /// The rows of a page indexed by code point (Bedrock's `default8.png`: cell n is U+00nn).
    pub fn code_page_rows() -> Vec<String> {
        (0..16).map(|row| (0..16).map(|col| char::from_u32(row * 16 + col).filter(|c| !c.is_control()).unwrap_or('\0')).collect()).collect()
    }

    fn advance(&self, c: char, bold: bool) -> f32 {
        if c == ' ' {
            return SPACE_ADVANCE;
        }
        let glyph = self.glyphs.get(&c).or_else(|| self.glyphs.get(&'?'));
        glyph.map_or(0.0, |g| g.width + 1.0 + f32::from(u8::from(bold)))
    }

    /// Width of `text` in GUI pixels, formatting codes left out.
    pub fn width(&self, text: &str) -> f32 {
        styled(text, 0xFFFFFF).map(|(c, s)| self.advance(c, s.bold)).sum()
    }

    /// Draws `text` with its top-left at (`x`, `y`) GUI pixels in `colour` (`0xRRGGBB`), `alpha`
    /// 0 to 1. Returns the width drawn.
    pub fn draw(&self, list: &mut DrawList, text: &str, x: f32, y: f32, colour: u32, alpha: f32, shadow: bool) -> f32 {
        if shadow {
            self.draw_pass(list, text, x + 1.0, y + 1.0, colour, alpha, true);
        }
        self.draw_pass(list, text, x, y, colour, alpha, false)
    }

    fn draw_pass(&self, list: &mut DrawList, text: &str, x: f32, y: f32, colour: u32, alpha: f32, shadow: bool) -> f32 {
        let mut pen = x;
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        for (c, style) in styled(text, colour) {
            let rgb = if shadow { darken(style.colour) } else { style.colour };
            let tint = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a];
            if let Some(g) = (c != ' ').then(|| self.glyphs.get(&c).or_else(|| self.glyphs.get(&'?'))).flatten() {
                let sprite = Sprite { x: self.page.x + g.cell[0], y: self.page.y + g.cell[1], width: self.cell, height: self.cell };
                let scale = GLYPH_HEIGHT / self.cell as f32;
                for dx in if style.bold { &[0.0, 1.0][..] } else { &[0.0][..] } {
                    list.sprite_stretched(sprite, [pen + dx, y, pen + dx + self.cell as f32 * scale, y + GLYPH_HEIGHT], tint);
                }
            }
            pen += self.advance(c, style.bold);
        }
        pen - x
    }
}

/// Java's shadow: each channel at a quarter.
fn darken(rgb: u32) -> u32 {
    (rgb & 0xFCFCFC) >> 2
}

#[derive(Debug, Clone, Copy)]
struct Style {
    colour: u32,
    bold: bool,
}

/// Each drawn character with its style; `§x` codes are consumed.
fn styled(text: &str, base: u32) -> impl Iterator<Item = (char, Style)> + '_ {
    let mut style = Style { colour: base, bold: false };
    let mut chars = text.chars();
    std::iter::from_fn(move || {
        loop {
            let c = chars.next()?;
            if c != '§' {
                return Some((c, style));
            }
            match chars.next()?.to_ascii_lowercase() {
                d @ ('0'..='9' | 'a'..='f') => style = Style { colour: COLOURS[d.to_digit(16)? as usize], bold: false },
                'l' => style.bold = true,
                'r' => style = Style { colour: base, bold: false },
                _ => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 128×128 page where 'A' is 5 pixels wide and 'i' 1.
    fn font() -> Font {
        let mut image = RgbaImage::new(128, 128);
        for (col, ink) in [(1u32, 5u32), (2, 1)] {
            for x in 0..ink {
                image.put_pixel(col * 8 + x, 3, image::Rgba([255; 4]));
            }
        }
        Font::from_grid(&image, Sprite { x: 0, y: 0, width: 128, height: 128 }, &["\0Ai"])
    }

    #[test]
    fn widths_follow_the_ink() {
        let f = font();
        assert_eq!(f.width("Ai"), 6.0 + 2.0);
        assert_eq!(f.width("A A"), 6.0 + 4.0 + 6.0);
        assert_eq!(f.width("§cA§li"), 6.0 + 3.0, "codes take no room, bold one more");
    }

    #[test]
    fn codes_colour_and_shadow_darkens() {
        let f = font();
        let mut list = DrawList::new(1.0);
        f.draw(&mut list, "A§aA", 0.0, 0.0, 0xFFFFFF, 1.0, true);
        let colours: Vec<[u8; 4]> = list.quads.iter().map(|q| q.color).collect();
        assert_eq!(colours, [[63, 63, 63, 255], [21, 63, 21, 255], [255, 255, 255, 255], [85, 255, 85, 255]]);
        assert_eq!(list.quads[2].rect, [0.0, 0.0, 8.0, 8.0]);
        assert_eq!(list.quads[3].rect[0], 6.0);
    }
}
