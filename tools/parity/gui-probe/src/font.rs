//! The `font` harness (`tools/parity/harnesses/font`): acacia-ui's Java fonts on the same cases as
//! `FontOracle`, the quads written as the oracle writes them (`draws` holds glyph quads, not calls).

use std::sync::{Arc, OnceLock};

use acacia_ui::java::font::{FontOptions, Fonts, GlyphQuad, GlyphTexture, Pen};
use acacia_ui::java::gui::{GuiDraw, Metrics};
use acacia_ui::java::mth::JavaRandom;
use acacia_ui::java::text::Text;
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

/// `Assets.font`: the default fonts, shared with this harness.
pub struct Shared(pub Arc<Fonts>);

impl Metrics for Shared {
    fn advance(&self, c: char, style: &acacia_ui::java::text::Style) -> f32 {
        self.0.advance(c, style)
    }
}
