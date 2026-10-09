//! The text on a sign as quads in font pixels around the text's centre, y down, for a renderer to
//! put in the world: Java's `AbstractSignRenderer` (26.3 client). Colours: README.md "Signs".

use crate::draw::{DrawList, Quad};
use crate::theme::Theme;

const LINES: usize = 4;
/// Java draws unlit text at this share of the dye's text colour.
const DARK: f32 = 0.4;
/// Java's outline around glowing black text (`BLACK_TEXT_OUTLINE_COLOR`).
const BLACK_OUTLINE: u32 = 0xF0EBCC;
/// Bedrock's `SignTextColor` after each dye, and Java's `DyeColor.getTextColor` for it.
const DYES: [(u32, u32); 16] = [
    (0xF0F0F0, 0xFFFFFF), (0xF9801D, 0xFF681F), (0xC74EBD, 0xFF00FF), (0x3AB3DA, 0x9AC0CD),
    (0xFED83D, 0xFFFF00), (0x80C71F, 0xBFFF00), (0xF38BAA, 0xFF69B4), (0x474F52, 0x808080),
    (0x9D9D97, 0xD3D3D3), (0x169C9C, 0x00FFFF), (0x8932B8, 0xA020F0), (0x3C44AA, 0x0000FF),
    (0x835432, 0x8B4513), (0x5E7C16, 0x00FF00), (0xB02E26, 0xFF0000), (0x1D1D21, 0x000000),
];

/// Line height and the widest line, in font pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub line_height: f32,
    pub max_width: f32,
}

pub const SIGN: Metrics = Metrics { line_height: 10.0, max_width: 90.0 };
pub const HANGING: Metrics = Metrics { line_height: 9.0, max_width: 60.0 };

/// One side of a sign as its block entity has it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Side {
    /// Lines separated by `\n`.
    pub text: String,
    /// `0xRRGGBB`.
    pub colour: u32,
    pub glowing: bool,
    /// Glowing text gets an outline.
    pub outline: bool,
}

/// A side's quads: `rect` in font pixels from the text's centre, `uv` in the theme's atlas.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Laid {
    pub text: Vec<Quad>,
    /// Drawn under the text; empty unless the side glows.
    pub outline: Vec<Quad>,
    /// Drawn at full brightness.
    pub glowing: bool,
    /// The outline shows at any distance (Java: around black text).
    pub outline_always: bool,
}

pub fn layout(theme: &Theme, side: &Side, metrics: Metrics) -> Laid {
    let Some(font) = &theme.font else { return Laid::default() };
    let colour = if theme.style.java_signs { DYES.iter().find(|d| d.0 == side.colour).map_or(side.colour, |d| d.1) } else { side.colour };
    let dark = if colour == 0 && side.glowing { BLACK_OUTLINE } else { scaled(colour, DARK) };
    let text_colour = if side.glowing || !theme.style.java_signs { colour } else { dark };
    let (mut text, mut outline) = (DrawList::new(1.0), DrawList::new(1.0));
    let lines = side.text.split('\n').flat_map(|line| font.wrap(line, metrics.max_width));
    for (i, line) in lines.take(LINES).enumerate() {
        // Java halves the negated integer width, rounding towards zero.
        let (x, y) = (-(font.width(&line) / 2.0).trunc(), (i as f32 - LINES as f32 / 2.0) * metrics.line_height);
        if side.glowing && side.outline {
            let plain = uncoloured(&line);
            for (dx, dy) in (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (dx, dy))).filter(|&d| d != (0, 0)) {
                font.draw(&mut outline, &plain, x + dx as f32, y + dy as f32, dark, 1.0, false);
            }
        }
        font.draw(&mut text, &line, x, y, text_colour, 1.0, false);
    }
    Laid { text: text.quads, outline: outline.quads, glowing: side.glowing, outline_always: colour == 0 }
}

fn scaled(rgb: u32, by: f32) -> u32 {
    let channel = |shift: u32| (((rgb >> shift) & 0xFF) as f32 * by) as u32;
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

/// `line` without its colour codes (bold stays: it widens glyphs), as the outline is one colour.
fn uncoloured(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('§', Some(code)) if !matches!(code, 'l' | 'L') => _ = chars.next(),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::test_theme::theme;

    fn side(text: &str, colour: u32, glowing: bool) -> Side {
        Side { text: text.into(), colour, glowing, outline: true }
    }

    #[test]
    fn lines_are_centred_from_the_top() {
        // Every glyph of the test font advances 6.
        let laid = layout(&theme(true), &side("ab\n\nabc", 0, false), SIGN);
        let tops: Vec<[f32; 2]> = laid.text.iter().map(|q| [q.rect[0], q.rect[1]]).collect();
        assert_eq!(tops, [[-6.0, -20.0], [0.0, -20.0], [-9.0, 0.0], [-3.0, 0.0], [3.0, 0.0]]);
        assert!(laid.outline.is_empty() && !laid.glowing);
        let five = layout(&theme(true), &side("a\nb\nc\nd\ne", 0, false), HANGING);
        assert_eq!(five.text.iter().map(|q| q.rect[1]).collect::<Vec<_>>(), [-18.0, -9.0, 0.0, 9.0]);
        let wrapped = layout(&theme(true), &side("aaaaaaa bbbbbbb ccccccc", 0, false), SIGN);
        assert_eq!(wrapped.text.iter().filter(|q| q.rect[1] == -10.0).count(), 7, "over 90 px wraps at a space");
    }

    #[test]
    fn looks_colour_dyed_and_glowing_text() {
        let red = |java: bool, glowing: bool| layout(&theme(java), &side("a", 0xB02E26, glowing), SIGN);
        assert_eq!(red(true, false).text[0].color, [102, 0, 0, 255], "Java: 40% of the dye's text colour");
        assert_eq!(red(false, false).text[0].color, [0xB0, 0x2E, 0x26, 255], "Bedrock: the stored colour");
        let glowing = red(true, true);
        assert_eq!((glowing.text[0].color, glowing.outline[0].color), ([255, 0, 0, 255], [102, 0, 0, 255]));
        assert_eq!((glowing.outline.len(), glowing.glowing, glowing.outline_always), (8, true, false));
        let black = layout(&theme(true), &side("§ca", 0, true), SIGN);
        assert_eq!((black.outline[0].color, black.outline_always), ([0xF0, 0xEB, 0xCC, 255], true));
        assert_eq!(black.text[0].color, [255, 85, 85, 255], "codes colour the text, not the outline");
        let hidden = layout(&theme(false), &Side { outline: false, ..side("a", 0, true) }, SIGN);
        assert!(hidden.outline.is_empty() && hidden.glowing);
    }
}
