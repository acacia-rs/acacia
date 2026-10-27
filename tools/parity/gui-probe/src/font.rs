//! The `font` harness (`tools/parity/harnesses/font`): acacia-ui's Java fonts on the same cases as
//! `FontOracle`, the quads written as the oracle writes them (`draws` holds glyph quads, not calls).

use std::sync::{Arc, OnceLock};

use acacia_ui::java::bidi::{VisualChar, props, visual_order};
use acacia_ui::java::font::{FontOptions, Fonts, GlyphQuad, GlyphTexture, Pen};
use acacia_ui::java::gui::{GuiDraw, Metrics};
use acacia_ui::java::mth::JavaRandom;
use acacia_ui::java::split::split;
use acacia_ui::java::text::{Style, Text};
use serde_json::{Map, Value, json};

use crate::Assets;

/// The fonts under each option set but the default one (`Assets.fonts`).
static OTHERS: [OnceLock<Fonts>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];

fn fonts<'a>(case: &Value, assets: &'a Assets) -> &'a Fonts {
    let flag = |k: &str| case[k].as_bool().unwrap_or(false);
    let options = FontOptions { uniform: flag("uniform"), japanese: flag("jp") };
    let i = usize::from(options.uniform) + 2 * usize::from(options.japanese);
    if i == 0 {
        return &assets.fonts;
    }
    OTHERS[i - 1].get_or_init(|| {
        let mut f = Fonts::java(&assets.root, assets.root.parent().and_then(|p| p.parent()), options);
        f.warnings.clear();
        f
    })
}

/// `GuiFonts.texture`'s key: Unifont glyphs by codepoint and `Arrays.hashCode` of their texels.
fn texture(fonts: &Fonts, t: &GlyphTexture) -> String {
    match t {
        GlyphTexture::Sheet(id) => id.to_string(),
        GlyphTexture::Unihex(_, c) => {
            let (_, _, rgba) = fonts.texels(t).unwrap_or_default();
            let hash = rgba.chunks(4).fold(1i32, |h, p| h.wrapping_mul(31).wrapping_add(if p[3] != 0 { -1 } else { 0 }));
            format!("unihex:{c:x}:{hash}")
        }
        GlyphTexture::Missing => "missing".into(),
        GlyphTexture::White => "white".into(),
    }
}

fn quad(fonts: &Fonts, q: &GlyphQuad) -> Value {
    let v: Vec<f32> = q.corners().iter().flatten().copied().collect();
    json!({"type": "glyph", "tex": texture(fonts, &q.texture), "colored": q.colored, "effect": q.effect, "color": q.color as i32, "z": q.z, "v": v})
}

pub fn replay(case: &Value, assets: &Assets, state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let fonts = fonts(case, assets);
    let draws = match case["kind"].as_str() {
        Some("glyphs") => {
            let set = fonts.set(case["font"].as_str());
            let fishy = case["fishy"].as_bool().unwrap_or(false);
            let (from, to) = (case["from"].as_u64().unwrap_or(0) as u32, case["to"].as_u64().unwrap_or(0) as u32);
            let mut list = Vec::new();
            for c in from..=to {
                let g = set.glyph(c, fishy);
                let mut e = vec![json!(c), json!(g.advance), json!(g.bold_offset), json!(g.shadow_offset)];
                match &g.bitmap {
                    Some(b) if b.texture == GlyphTexture::Missing => continue,
                    Some(b) => e.extend([json!(texture(fonts, &b.texture)), json!(b.colored), json!(b.left), json!(b.right), json!(b.up), json!(b.down)]),
                    None => e.push(Value::Null),
                }
                list.push(Value::Array(e));
            }
            state.insert("glyphs".into(), Value::Array(list));
            Vec::new()
        }
        Some("bywidth") => {
            let set = fonts.set(case["font"].as_str());
            let by: Map<String, Value> = set.by_width().iter().map(|(w, l)| (w.to_string(), json!(l))).collect();
            state.insert("by_width".into(), Value::Object(by));
            Vec::new()
        }
        Some("visual") => chars(&visual_order(&Text::from_json(&case["text"], &|_| None))),
        Some("split") => {
            let text = Text::from_json(&case["text"], &|_| None);
            let lines = split(fonts, &text, case["width"].as_i64().unwrap_or(0) as i32);
            lines.iter().flat_map(|line| {
                let mut out = vec![json!({"type": "line", "width": fonts.visual_width(line)})];
                out.extend(chars(&visual_order(line)));
                out
            }).collect()
        }
        Some("props") => {
            let (from, to) = (case["from"].as_u64().unwrap_or(0) as u32, case["to"].as_u64().unwrap_or(0) as u32);
            let list: Vec<Value> = (from..=to)
                .map(|c| (c, props::class(c), props::mirror(c), props::bracket_type(c)))
                .filter(|&(c, class, mirror, bracket)| class != 0 || mirror != c || bracket != 0)
                .map(|(c, class, mirror, bracket)| json!([c, class, mirror, bracket]))
                .collect();
            state.insert("props".into(), Value::Array(list));
            Vec::new()
        }
        _ => {
            let text = Text::from_json(&case["text"], &|_| None);
            state.insert("width".into(), json!(fonts.width(&text)));
            let f = |k: &str| case[k].as_f64().unwrap_or(0.0) as f32;
            let pen = Pen {
                background: case["bg"].as_i64().unwrap_or(0) as u32,
                filter_fishy: case["fishy"].as_bool().unwrap_or(false),
                ..Pen::gui(f("x"), f("y"), case["color"].as_i64().unwrap_or(-1) as u32, case["shadow"].as_bool().unwrap_or(false))
            };
            let mut random = JavaRandom::new(case["seed"].as_i64().unwrap_or(0));
            let prepared = fonts.prepare(&text, pen, &mut random);
            state.insert("bounds".into(), json!(prepared.bounds));
            prepared.quads.iter().map(|q| quad(fonts, q)).collect()
        }
    };
    state.insert("draws".into(), Value::Array(draws));
    Vec::new()
}

/// `FontOracle.styleKey`.
fn style_key(s: &Style) -> Value {
    let font = s.font.as_deref().filter(|f| *f != "minecraft:default");
    json!([s.color, s.is_bold(), s.is_italic(), s.is_underlined(), s.is_strikethrough(), s.is_obfuscated(), font, s.shadow_color.map(|c| c as i32)])
}

/// `FontOracle.chars`.
fn chars(seq: &[VisualChar]) -> Vec<Value> {
    seq.iter().map(|v| json!({"type": "char", "cp": v.codepoint, "first": v.first, "style": style_key(&v.style)})).collect()
}

/// `Assets.font`: the default fonts, shared with this harness.
pub struct Shared(pub Arc<Fonts>);

impl Metrics for Shared {
    fn advance(&self, c: char, style: &Style) -> f32 {
        self.0.advance(c, style)
    }

    fn active_areas(&self, text: &Text, x: f32, y: f32, color: u32, shadow: bool) -> acacia_ui::java::active::TextAreas {
        self.0.active_areas(text, x, y, color, shadow)
    }
}
