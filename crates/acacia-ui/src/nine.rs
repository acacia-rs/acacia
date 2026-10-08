//! Nine-slice sprites: corners kept, edges and centre filled to any size. Java tiles them
//! (`blitNineSlicedSprite`), Bedrock stretches them; the border comes from the sprite's `.mcmeta`
//! (Java) or its `.json` beside the PNG (Bedrock, `nineslice_size`).

use std::path::Path;

use crate::atlas::{Atlas, Sprite};
use crate::draw::DrawList;
use crate::theme::png;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nine {
    pub sprite: Sprite,
    /// Left, top, right, bottom, in texels (one texel is one GUI pixel).
    pub border: [f32; 4],
    /// Edges and centre repeat instead of stretching.
    pub tile: bool,
}

impl Nine {
    /// A Bedrock `textures/ui` image, `name` without extension, with its `.json` border.
    pub fn bedrock(root: &Path, name: &str, atlas: &mut Atlas) -> Option<Nine> {
        let base = root.join("textures/ui").join(name);
        let sprite = atlas.add(&format!("ui/{name}"), &png(&base.with_extension("png"))?);
        let json: serde_json::Value = std::fs::read_to_string(base.with_extension("json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let border = match &json["nineslice_size"] {
            serde_json::Value::Number(n) => [n.as_f64().unwrap_or(0.0) as f32; 4],
            serde_json::Value::Array(a) if a.len() == 4 => std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32),
            _ => [0.0; 4],
        };
        Some(Nine { sprite, border, tile: false })
    }

    /// A Java `textures/gui/sprites` image, `name` without extension, with its `.mcmeta` border.
    pub fn java(root: &Path, name: &str, atlas: &mut Atlas) -> Option<Nine> {
        let base = root.join("textures/gui/sprites").join(name);
        let sprite = atlas.add(&format!("sprites/{name}"), &png(&base.with_extension("png"))?);
        let meta: serde_json::Value = std::fs::read_to_string(base.with_extension("png.mcmeta")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let b = &meta["gui"]["scaling"]["border"];
        let side = |k: &str| b[k].as_f64().unwrap_or(0.0) as f32;
        let border = match b.as_f64() {
            Some(n) => [n as f32; 4],
            None => [side("left"), side("top"), side("right"), side("bottom")],
        };
        Some(Nine { sprite, border, tile: true })
    }
}

impl DrawList {
    /// `nine` filling `rect` (GUI pixels).
    pub fn nine(&mut self, nine: Nine, rect: [f32; 4], color: [u8; 4]) {
        let [l, t, r, b] = rect;
        let (sw, sh) = (nine.sprite.width as f32, nine.sprite.height as f32);
        let [bl, bt, br, bb] = nine.border;
        let (bl, br) = (bl.min(sw / 2.0).min((r - l) / 2.0), br.min(sw / 2.0).min((r - l) / 2.0));
        let (bt, bb) = (bt.min(sh / 2.0).min((b - t) / 2.0), bb.min(sh / 2.0).min((b - t) / 2.0));
        let cols = [(l, l + bl, 0.0, bl), (l + bl, r - br, bl, sw - br), (r - br, r, sw - br, sw)];
        let rows = [(t, t + bt, 0.0, bt), (t + bt, b - bb, bt, sh - bb), (b - bb, b, sh - bb, sh)];
        for (ci, &(x0, x1, u0, u1)) in cols.iter().enumerate() {
            for (ri, &(y0, y1, v0, v1)) in rows.iter().enumerate() {
                if x1 <= x0 || y1 <= y0 || u1 <= u0 || v1 <= v0 {
                    continue;
                }
                let tile_x = nine.tile && ci == 1;
                let tile_y = nine.tile && ri == 1;
                self.cell(nine.sprite, [x0, y0, x1, y1], [u0, v0, u1, v1], [tile_x, tile_y], color);
            }
        }
    }

    /// The `part` of `sprite` over `rect`, repeated along the axes `tile` names, else stretched.
    fn cell(&mut self, sprite: Sprite, rect: [f32; 4], part: [f32; 4], tile: [bool; 2], color: [u8; 4]) {
        let [x0, y0, x1, y1] = rect;
        let [u0, v0, u1, v1] = part;
        let step_x = if tile[0] { u1 - u0 } else { x1 - x0 };
        let step_y = if tile[1] { v1 - v0 } else { y1 - y0 };
        let mut y = y0;
        while y < y1 {
            let h = step_y.min(y1 - y);
            let vb = if tile[1] { v0 + h } else { v1 };
            let mut x = x0;
            while x < x1 {
                let w = step_x.min(x1 - x);
                let ur = if tile[0] { u0 + w } else { u1 };
                let (u, v) = (sprite.x as f32, sprite.y as f32);
                self.quad([x, y, x + w, y + h], [u + u0, v + v0, u + ur, v + vb], color);
                x += w;
            }
            y += h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::WHITE;

    fn nine(tile: bool) -> Nine {
        Nine { sprite: Sprite { x: 0, y: 0, width: 200, height: 20 }, border: [3.0; 4], tile }
    }

    #[test]
    fn at_its_own_size_it_is_the_sprite() {
        let mut list = DrawList::new(1.0);
        list.nine(nine(true), [0.0, 0.0, 200.0, 20.0], WHITE);
        let area: f32 = list.quads.iter().map(|q| (q.rect[2] - q.rect[0]) * (q.rect[3] - q.rect[1])).sum();
        assert_eq!(area, 4000.0);
        assert!(list.quads.iter().all(|q| q.uv[2] - q.uv[0] == q.rect[2] - q.rect[0]), "texel for pixel");
    }

    #[test]
    fn narrower_keeps_the_right_cap_and_tiles_wider() {
        let mut list = DrawList::new(1.0);
        list.nine(nine(true), [0.0, 0.0, 150.0, 20.0], WHITE);
        let right = list.quads.iter().find(|q| q.rect[0] == 147.0).unwrap();
        assert_eq!(right.uv[0], 197.0, "Java shows the sprite's own right edge");
        let mut list = DrawList::new(1.0);
        list.nine(nine(true), [0.0, 0.0, 400.0, 20.0], WHITE);
        assert!(list.quads.iter().all(|q| q.uv[2] - q.uv[0] == q.rect[2] - q.rect[0]), "tiled, never stretched");
        let mut list = DrawList::new(1.0);
        list.nine(nine(false), [0.0, 0.0, 400.0, 20.0], WHITE);
        assert_eq!(list.quads.len(), 9, "Bedrock stretches each cell once");
    }
}
