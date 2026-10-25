//! `Font.prepareText` and the glyph render states, CPU-side: styled text at a pen position becomes
//! the quads Java's `PreparedTextBuilder` visits, in its order: the background, then per glyph its
//! shadow (and the shadow's bold copy) and itself (and its bold copy), then each underline and
//! strikethrough with its shadow. Coordinates are GUI pixels before the pose.

use super::provider::{Bitmap, GlyphTexture};
use super::{Fonts, VisualChar};
use crate::java::active::ActiveArea;
use crate::atlas::Sprite;
use crate::draw::Quad;
use crate::java::paint::{Glyphs, argb};
use crate::java::mth::{JavaRandom, alpha, as8, ceil, scale_rgb, with_alpha};
use crate::java::text::Text;

/// One quad as `BakedSheetGlyph.render` / `buildEffect` emits it.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphQuad {
    pub texture: GlyphTexture,
    /// `GUI_TEXT` (true) or `GUI_TEXT_GRAYSCALE`.
    pub colored: bool,
    /// Left, top, right, bottom, before the italic shear.
    pub rect: [f32; 4],
    /// How far right the top and bottom edges are shifted (italic).
    pub skew: [f32; 2],
    /// Left, top, right, bottom texel coordinates within `texture`, nudged 0.01 inwards as
    /// `FontTexture.add` nudges them.
    pub uv: [f32; 4],
    pub z: f32,
    pub color: u32,
    /// An effect rectangle (underline, strikethrough, background): its corners take the white
    /// glyph's UVs in `buildEffect`'s order instead of a glyph's.
    pub effect: bool,
}

impl GlyphQuad {
    /// The four vertices in Java's order, each `[x, y, u, v]`.
    pub fn corners(&self) -> [[f32; 4]; 4] {
        let [l, t, r, b] = self.rect;
        let [u0, v0, u1, v1] = self.uv;
        if self.effect {
            return [[l, b, u0, v0], [r, b, u0, v1], [r, t, u1, v1], [l, t, u1, v0]];
        }
        let [st, sb] = self.skew;
        [[l + st, t, u0, v0], [l + sb, b, u0, v1], [r + sb, b, u1, v1], [r + st, t, u1, v0]]
    }
}

/// What `prepareText` gives: the quads and `PreparedTextBuilder`'s float bounds (left, top, right,
/// bottom), `None` when empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prepared {
    pub quads: Vec<GlyphQuad>,
    pub bounds: Option<[f32; 4]>,
    /// The glyphs' active areas, then (as with `includeEmpty`) the empty glyphs'.
    pub areas: Vec<ActiveArea>,
}

impl Prepared {
    /// The painter's quads (`paint::Source::text`): each glyph's texture placed in the atlas by
    /// `sprite` (a whole sheet, one Unifont glyph or a special glyph; `Fonts::texels`). Quads whose
    /// texture has no sprite are left out.
    pub fn glyphs(&self, mut sprite: impl FnMut(&GlyphTexture) -> Option<Sprite>) -> Glyphs {
        let quads = self.quads.iter().filter_map(|q| {
            let s = sprite(&q.texture)?;
            let (x, y) = (s.x as f32, s.y as f32);
            let uv = [x + q.uv[0], y + q.uv[1], x + q.uv[2], y + q.uv[3]];
            Some(Quad { skew: q.skew, ..Quad::plain(q.rect, uv, argb(q.color)) })
        });
        Glyphs { quads: quads.collect(), bounds: self.bounds }
    }
}

/// How text is drawn: `flat` is true in the GUI (depths stay 0), false in the world.
#[derive(Clone, Copy, Debug)]
pub struct Pen {
    pub x: f32,
    pub y: f32,
    pub color: u32,
    pub shadow: bool,
    pub background: u32,
    pub flat: bool,
    /// `Minecraft.fontFilterFishy`: glyphs with advances outside 0..=32 skipped.
    pub filter_fishy: bool,
}

impl Pen {
    pub fn gui(x: f32, y: f32, color: u32, shadow: bool) -> Pen {
        Pen { x, y, color, shadow, background: 0, flat: true, filter_fishy: false }
    }
}

fn uv(b: &Bitmap) -> [f32; 4] {
    let [x, y, w, h] = b.src.map(|v| v as f32);
    [x + 0.01, y + 0.01, x - 0.01 + w, y - 0.01 + h]
}

struct Builder {
    bounds: [f32; 4],
    background: [f32; 4],
    glyphs: Vec<GlyphQuad>,
    effects: Vec<GlyphQuad>,
    areas: Vec<ActiveArea>,
    /// `includeEmpty`'s areas, visited after the glyphs.
    empty: Vec<ActiveArea>,
}

impl Builder {
    fn mark(&mut self, l: f32, t: f32, r: f32, b: f32) {
        let s = &mut self.bounds;
        *s = [s[0].min(l), s[1].min(t), s[2].max(r), s[3].max(b)];
    }
}

/// `createEffect(x0, y0, x1, y1, depth, color, shadowColor, shadowOffset)` and its rendering
/// (effects always render as not flat).
fn effect(fonts: &Fonts, rect: [f32; 4], depth: f32, color: u32, shadow: u32, offset: f32, out: &mut Vec<GlyphQuad>) {
    let white = fonts.set(None).white.bitmap.clone().expect("the white glyph has a bitmap");
    let quad = |o: f32, z: f32, color: u32| GlyphQuad {
        texture: GlyphTexture::White,
        colored: true,
        rect: [rect[0] + o, rect[1] + o, rect[2] + o, rect[3] + o],
        skew: [0.0; 2],
        uv: uv(&white),
        z,
        color,
        effect: true,
    };
    let mut z = depth;
    if shadow != 0 {
        out.push(quad(offset, z, shadow));
        z += 0.03;
    }
    out.push(quad(0.0, z, color));
}

impl Fonts {
    /// `Font.prepareText(text, x, y, color, shadow, includeEmpty = false, background)` and the
    /// quads its glyphs and effects render; `random` picks obfuscated glyphs (`Font.random`).
    pub fn prepare(&self, text: &Text, pen: Pen, random: &mut JavaRandom) -> Prepared {
        let mut b = Builder {
            bounds: [f32::MAX, f32::MAX, -f32::MAX, -f32::MAX],
            background: [f32::MAX, f32::MAX, -f32::MAX, -f32::MAX],
            glyphs: Vec::new(),
            effects: Vec::new(),
            areas: Vec::new(),
            empty: Vec::new(),
        };
        let has_background = alpha(pen.background) != 0;
        let mark_background = |b: &mut Builder, x: f32, y: f32, advance: f32| {
            if has_background {
                let g = &mut b.background;
                *g = [g[0].min(x - 1.0), g[1].min(y - 1.0), g[2].max(x + advance), g[3].max(y + 9.0)];
                let g = *g;
                b.mark(g[0], g[1], g[2], g[3]);
            }
        };
        let (mut x, y) = (pen.x, pen.y);
        mark_background(&mut b, x, y, 0.0);
        for VisualChar { first, style, codepoint } in self.visual_order(text) {
            let set = self.set(style.font.as_deref());
            let mut glyph = set.glyph(codepoint, pen.filter_fishy);
            if style.is_obfuscated() && codepoint != 32 {
                glyph = set.random_glyph(random, ceil(glyph.advance(false)));
            }
            let bold = style.is_bold();
            let color = style.color.map_or(pen.color, |rgb| with_alpha(alpha(pen.color), rgb));
            let shadow = match style.shadow_color {
                Some(s) if alpha(color) != 255 => with_alpha(as8(alpha(color) as f32 / 255.0 * (alpha(s) as f32 / 255.0)), s),
                Some(s) => s,
                None if pen.shadow => scale_rgb(color, 0.25),
                None => 0,
            };
            let advance = glyph.advance(bold);
            let effect_x0 = if first { x - 1.0 } else { x };
            let shadow_offset = glyph.shadow_offset;
            let bold_offset = if bold { glyph.bold_offset } else { 0.0 };
            if let Some(bitmap) = &glyph.bitmap {
                let italic = style.is_italic();
                let thick = if bold { 0.1 } else { 0.0 };
                let (shear_top, shear_bottom) = (1.0 - 0.25 * bitmap.up, 1.0 - 0.25 * bitmap.down);
                let has_shadow = shadow != 0;
                let left = x + bitmap.left + if italic { shear_top.min(shear_bottom) } else { 0.0 } - thick;
                let top = y + bitmap.up - thick;
                let right = x + bitmap.right + if has_shadow { shadow_offset } else { 0.0 } + if italic { shear_top.max(shear_bottom) } else { 0.0 } + thick;
                let bottom = y + bitmap.down + if has_shadow { shadow_offset } else { 0.0 } + thick;
                b.mark(left, top, right, bottom);
                b.areas.push(ActiveArea { rect: [left, top, x + advance, bottom], style: style.clone() });
                let render = |x: f32, y: f32, z: f32, color: u32, bold: bool| {
                    let e = if bold { 0.1 } else { 0.0 };
                    GlyphQuad {
                        texture: bitmap.texture.clone(),
                        colored: bitmap.colored,
                        rect: [x + bitmap.left - e, y + bitmap.up - e, x + bitmap.right + e, y + bitmap.down + e],
                        skew: if italic { [shear_top, shear_bottom] } else { [0.0; 2] },
                        uv: uv(bitmap),
                        z,
                        color,
                        effect: false,
                    }
                };
                let z_fighter = if pen.flat { 0.0 } else { 0.001 };
                let mut depth = 0.0;
                if has_shadow {
                    b.glyphs.push(render(x + shadow_offset, y + shadow_offset, 0.0, shadow, bold));
                    if bold {
                        b.glyphs.push(render(x + bold_offset + shadow_offset, y + shadow_offset, z_fighter, shadow, true));
                    }
                    depth = if pen.flat { 0.0 } else { 0.03 };
                }
                b.glyphs.push(render(x, y, depth, color, bold));
                if bold {
                    b.glyphs.push(render(x + bold_offset, y, depth + z_fighter, color, true));
                }
            } else {
                b.empty.push(ActiveArea { rect: [x, y, x + advance, y + 9.0], style: style.clone() });
            }
            mark_background(&mut b, x, y, advance);
            let mut effects = Vec::new();
            if style.is_strikethrough() {
                effects.push([effect_x0, y + 4.5 - 1.0, x + advance, y + 4.5]);
            }
            if style.is_underlined() {
                effects.push([effect_x0, y + 9.0 - 1.0, x + advance, y + 9.0]);
            }
            for r in effects {
                b.mark(r[0], r[1], r[2] + if shadow != 0 { shadow_offset } else { 0.0 }, r[3] + if shadow != 0 { shadow_offset } else { 0.0 });
                let mut out = std::mem::take(&mut b.effects);
                effect(self, r, 0.01, color, shadow, shadow_offset, &mut out);
                b.effects = out;
            }
            x += advance;
        }
        let mut quads = Vec::with_capacity(b.glyphs.len() + b.effects.len() + 1);
        if has_background {
            effect(self, b.background, -0.01, pen.background, 0, 0.0, &mut quads);
        }
        quads.append(&mut b.glyphs);
        quads.append(&mut b.effects);
        let [l, t, r, bt] = b.bounds;
        b.areas.append(&mut b.empty);
        Prepared { quads, bounds: (l < r && t < bt).then_some(b.bounds), areas: b.areas }
    }
}
