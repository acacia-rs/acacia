//! `gui-probe frame:<harness>` (the `gui_frame` harness): a harness's cases replayed as its own
//! module replays them, then painted by `acacia_ui::java::paint` at GUI scale 1 over a [`Names`]
//! source. Each output line is `{"frame": [...]}`, the painted quads in order, each mapped back to
//! what it samples: `{k: blit|fill|glyph|item, t, uv (sprite-local), c (ARGB top[, bottom]), b
//! (blend), ab (after the blur), r (rect), s (italic skew)}`. Decorations and pictures are left out.

use std::collections::HashMap;

use acacia_ui::draw::Blend;
use acacia_ui::java::font::Pen;
use acacia_ui::java::gui::{GuiDraw, Metrics};
use acacia_ui::java::mth::JavaRandom;
use acacia_ui::java::paint::assets::{self, Images};
use acacia_ui::java::paint::blit::Scaling;
use acacia_ui::java::paint::{self, Glyphs, Source};
use acacia_ui::java::picture::Picture;
use acacia_ui::java::text::Text;
use acacia_ui::{Quad, Sprite};
use serde_json::{Value, json};

use crate::Assets;

/// FrameOracle's `Util.getMillis()`.
const MILLIS: i64 = 1_234_567;
/// Every named texture gets a `SIZE`-texel square at `x = 0`, one per `STRIDE` rows, so a texel
/// maps back to its name and sprite-local UV even where a repeating texture's UVs run past 1;
/// items and glyphs sit in columns right of them.
const STRIDE: u32 = 8192;
const SIZE: u32 = 64;
const ITEM_X: u32 = 1 << 20;
const GLYPH_X: u32 = 1 << 21;

/// A source that names what each quad samples instead of packing images.
pub struct Names<'a> {
    assets: &'a Assets,
    names: Vec<String>,
    index: HashMap<String, usize>,
    scaling: HashMap<String, Option<Scaling>>,
    images: Images,
    random: JavaRandom,
}

impl<'a> Names<'a> {
    pub fn new(assets: &'a Assets) -> Names<'a> {
        let images = Images::new(&assets.root);
        Names { assets, names: Vec::new(), index: HashMap::new(), scaling: HashMap::new(), images, random: JavaRandom::new(0) }
    }

    fn region(&mut self, name: String) -> Sprite {
        let i = match self.index.get(&name) {
            Some(&i) => i,
            None => {
                self.names.push(name.clone());
                self.index.insert(name, self.names.len() - 1);
                self.names.len() - 1
            }
        } as u32;
        Sprite { x: 0, y: i * STRIDE, width: SIZE, height: SIZE }
    }

    fn exists(&self, path: &str) -> bool {
        self.assets.root.join("textures").join(format!("{path}.png")).is_file()
    }

    /// One painted quad as the comparison reads it.
    fn describe(&self, q: &Quad, after_blur: bool) -> Value {
        let r = q.rect.map(round);
        let [ul, vt, ur, vb] = q.uv;
        let left = ul.min(ur);
        if left >= GLYPH_X as f32 {
            return json!({"k": "glyph", "r": r, "c": [argb(q.color)], "s": q.skew.map(round), "ab": after_blur});
        }
        if left >= ITEM_X as f32 {
            return json!({"k": "item", "r": r, "ab": after_blur});
        }
        let row = (vt.min(vb) / STRIDE as f32).floor() as usize;
        let name = self.names.get(row).map_or("?", String::as_str);
        let (ox, oy) = (0.0, (row as u32 * STRIDE) as f32);
        if name == "fill" {
            return json!({"k": "fill", "r": r, "c": [argb(q.color), argb(q.bottom)], "b": blend(q.blend), "ab": after_blur});
        }
        let uv = [(ul - ox) / SIZE as f32, (vt - oy) / SIZE as f32, (ur - ox) / SIZE as f32, (vb - oy) / SIZE as f32].map(round);
        json!({"k": "blit", "t": name, "uv": uv, "r": r, "c": [argb(q.color)], "b": blend(q.blend), "ab": after_blur})
    }
}

fn round(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

fn argb([r, g, b, a]: [u8; 4]) -> u32 {
    u32::from_be_bytes([a, r, g, b])
}

fn blend(b: Blend) -> &'static str {
    match b {
        Blend::Alpha | Blend::Premultiplied => "alpha",
        Blend::Invert => "invert",
        Blend::Darken => "darken",
        Blend::Add => "add",
    }
}

fn path(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

impl Source for Names<'_> {
    fn sprite(&mut self, id: &str) -> Option<(Sprite, Scaling)> {
        let id = path(id);
        let scaling = match self.scaling.get(id) {
            Some(s) => *s,
            None => {
                let base = self.images.sprite_file(id).unwrap_or_default();
                let meta = std::fs::read_to_string(base.with_extension("png.mcmeta")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
                let s = (base.with_extension("png").is_file() || id == assets::MISSING).then(|| Scaling::from_mcmeta(&meta));
                self.scaling.insert(id.to_owned(), s);
                s
            }
        }?;
        Some((self.region(format!("sprite:{id}")), scaling))
    }

    fn block_sprite(&mut self, id: &str) -> Option<Sprite> {
        let id = path(id);
        // FrameOracle's block atlas doesn't name its sprites.
        self.exists(id).then(|| self.region("atlas".into()))
    }

    fn texture(&mut self, p: &str) -> Option<Sprite> {
        let p = path(p);
        self.exists(p).then(|| self.region(format!("tex:{p}")))
    }

    fn white(&mut self) -> Sprite {
        self.region("fill".into())
    }

    fn text(&mut self, text: &Text, x: i32, y: i32, color: u32, shadow: bool) -> Glyphs {
        let prepared = self.assets.fonts.prepare(text, Pen::gui(x as f32, y as f32, color, shadow), &mut self.random);
        prepared.glyphs(|_| Some(Sprite { x: GLYPH_X, y: 0, width: 1, height: 1 }))
    }

    fn width(&mut self, text: &Text) -> i32 {
        Metrics::width(&*self.assets.fonts, text)
    }

    fn millis(&self) -> i64 {
        MILLIS
    }

    fn item(&mut self, _: i32, x: i32, y: i32, _: i32) -> Vec<Quad> {
        let (x, y, u) = (x as f32, y as f32, ITEM_X as f32);
        vec![Quad::plain([x, y, x + 16.0, y + 16.0], [u, 0.0, u + 16.0, 16.0], [255; 4])]
    }

    fn decorations(&mut self, _: i32, _: i32, _: i32, _: Option<&str>) -> Vec<GuiDraw> {
        Vec::new()
    }

    fn atlas_sprite(&mut self, _: &str, _: &str) -> Option<Sprite> {
        None
    }

    fn picture(&mut self, _: &Picture, _: [f32; 4]) -> Option<Quad> {
        None
    }

    fn picture_draws(&mut self, _: &Picture) -> Vec<GuiDraw> {
        Vec::new()
    }
}

/// The frame `draws` paint for `case` (its `width` × `height` GUI).
pub fn frame(case: &Value, draws: Vec<GuiDraw>, assets: &Assets) -> Value {
    let draws: Vec<GuiDraw> = draws.into_iter().filter(|d| !matches!(d, GuiDraw::Decorations { .. } | GuiDraw::Picture(_))).collect();
    let size = |k: &str, d: i64| case[k].as_i64().unwrap_or(d) as i32;
    let mut names = Names::new(assets);
    let painted = paint::paint(&draws, [size("width", 427), size("height", 240)], 1.0, &mut names);
    let blur = painted.blur_at.unwrap_or(usize::MAX);
    let quads: Vec<Value> = painted.quads.iter().enumerate().map(|(i, q)| names.describe(q, i >= blur)).collect();
    json!({"frame": quads})
}
