//! Draw calls as the parity oracle records them (one JSON object per call, poses folded into a
//! `matrix` on each call that is not drawn at the identity). The game's side writes the same objects:
//! `tools/parity/java/acacia/parity/gui/GuiCapture.java`.

use serde_json::{Value, json};

use super::gui::{GuiDraw, IDENTITY, Pose, pose_apply};
use super::text::Text;

/// Style runs `[text, rgb|null, bold, italic, underlined, strikethrough, obfuscated, font|null]`
/// per run of equally styled characters, or `None` for unstyled text.
pub fn runs(t: &Text) -> Option<Value> {
    type Key = (Option<u32>, bool, bool, bool, bool, bool, Option<String>);
    let mut out: Vec<(String, Key)> = Vec::new();
    let mut styled = false;
    for (ch, s) in t.chars() {
        let key: Key = (s.color, s.is_bold(), s.is_italic(), s.is_underlined(), s.is_strikethrough(), s.is_obfuscated(), s.font.clone().filter(|f| f != "minecraft:default"));
        styled |= key != (None, false, false, false, false, false, None);
        match out.last_mut() {
            Some(last) if last.1 == key => last.0.push(ch),
            _ => out.push((ch.to_string(), key)),
        }
    }
    styled.then(|| json!(out.into_iter().map(|(s, k)| json!([s, k.0, k.1, k.2, k.3, k.4, k.5, k.6])).collect::<Vec<_>>()))
}

pub fn encode(draws: &[GuiDraw]) -> Vec<Value> {
    let mut stack: Vec<Pose> = vec![IDENTITY];
    let mut out = Vec::new();
    for d in draws {
        let pose = *stack.last().unwrap_or(&IDENTITY);
        let mut v = match d {
            GuiDraw::Push(ops) => {
                let mut m = pose;
                for op in ops {
                    pose_apply(&mut m, *op);
                }
                stack.push(m);
                continue;
            }
            GuiDraw::Pop => {
                if stack.len() > 1 {
                    stack.pop();
                }
                continue;
            }
            GuiDraw::Sprite { sprite, x, y, w, h, region, color, blend } => {
                json!({"type": "sprite", "sprite": sprite, "x": x, "y": y, "w": w, "h": h, "region": region, "color": *color as i32, "pipeline": blend.pipeline()})
            }
            GuiDraw::AtlasSprite { sprite, x, y, w, h, color } => json!({"type": "atlas", "sprite": sprite, "x": x, "y": y, "w": w, "h": h, "color": *color as i32}),
            GuiDraw::Blit { texture, blend, x, y, w, h, uv, src, tex, color } => json!({
                "type": "blit", "texture": format!("textures/{texture}.png"), "pipeline": blend.pipeline(),
                "x": x, "y": y, "w": w, "h": h, "uv": uv, "src": src, "tex": tex, "color": *color as i32}),
            GuiDraw::Fill { x0, y0, x1, y1, color } => json!({"type": "fill", "x0": x0, "y0": y0, "x1": x1, "y1": y1, "color": *color as i32}),
            GuiDraw::Gradient { x0, y0, x1, y1, top, bottom } => {
                json!({"type": "gradient", "x0": x0, "y0": y0, "x1": x1, "y1": y1, "top": *top as i32, "bottom": *bottom as i32})
            }
            GuiDraw::Text { text, x, y, color, shadow } => {
                let text = text.resolve_legacy();
                let mut v = json!({"type": "text", "text": text.plain(), "x": x, "y": y, "color": *color as i32, "shadow": shadow});
                if let Some(r) = runs(&text) {
                    v["runs"] = r;
                }
                v
            }
            GuiDraw::Item { key, x, y, seed } => {
                out.push(json!({"type": "item", "slot": key, "x": x, "y": y, "seed": seed, "matrix": pose}));
                continue;
            }
            GuiDraw::Decorations { key, x, y, count } => json!({"type": "decorations", "slot": key, "x": x, "y": y, "count": count}),
            GuiDraw::Scissor { x0, y0, x1, y1 } => json!({"type": "scissor", "x0": x0, "y0": y0, "x1": x1, "y1": y1}),
            GuiDraw::Unscissor => json!({"type": "unscissor"}),
            GuiDraw::Scrolling { text, center_x, x0, x1, y0, y1, color } => {
                let text = text.resolve_legacy();
                let mut v = json!({"type": "scrolling", "text": text.plain(), "centerX": center_x, "x0": x0, "x1": x1, "y0": y0, "y1": y1, "color": *color as i32});
                if let Some(r) = runs(&text) {
                    v["runs"] = r;
                }
                v
            }
            GuiDraw::Highlight { x0, y0, x1, y1, invert } => {
                let fill = |pipeline: &str, color: i32| {
                    let mut v = json!({"type": "fill", "x0": x0.min(x1), "y0": y0.min(y1), "x1": x0.max(x1), "y1": y0.max(y1), "color": color, "pipeline": pipeline});
                    if pose != IDENTITY {
                        v["matrix"] = json!(pose);
                    }
                    v
                };
                if *invert {
                    out.push(fill("pipeline/gui_invert", -1));
                }
                out.push(fill("pipeline/gui_text_highlight", -16_776_961));
                continue;
            }
            // GuiCapture records these two without the pose.
            GuiDraw::Stratum | GuiDraw::Blur => {
                out.push(json!({"type": if matches!(d, GuiDraw::Stratum) { "stratum" } else { "blur" }}));
                continue;
            }
            GuiDraw::Picture(p) => p.encode(),
        };
        if pose != IDENTITY {
            v["matrix"] = json!(pose);
        }
        out.push(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::{Gui, PoseOp};

    #[test]
    fn poses_fold_into_the_calls_under_them() {
        let mut gui = Gui::new(320, 240);
        gui.push(vec![PoseOp::Translate(10.0, 0.0), PoseOp::Scale(2.0, 2.0)]);
        gui.sprite("hud/air", 1, 2, 9, 9);
        gui.item(3, 0, 0, 1);
        gui.pop();
        gui.text(Text::legacy("§cA"), 0, 0, 0xFF00_0000, false);
        let v = encode(&gui.draws);
        assert_eq!(v[0]["matrix"], json!([2.0, 0.0, 0.0, 2.0, 10.0, 0.0]));
        assert_eq!(v[1]["matrix"], json!([2.0, 0.0, 0.0, 2.0, 10.0, 0.0]));
        assert!(v[2].get("matrix").is_none());
        assert_eq!(v[2]["runs"], json!([["A", 0xFF5555, false, false, false, false, false, null]]));
    }
}
