//! `GuiGraphicsExtractor.blitSprite` below its public calls: a GUI sprite's `.mcmeta` scaling
//! (stretch, tile, nine-slice) turns one call into the blits the game adds, each an element of
//! its own for the layering. UVs are fractions of the sprite (`TextureAtlasSprite.getU`).

use serde_json::Value;

/// `GuiSpriteScaling`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scaling {
    Stretch,
    Tile { w: i32, h: i32 },
    NineSlice { w: i32, h: i32, border: [i32; 4], stretch_inner: bool },
}

impl Scaling {
    /// From a sprite's `.png.mcmeta` (`gui.scaling`); stretch when there is none.
    pub fn from_mcmeta(meta: &Value) -> Scaling {
        let s = &meta["gui"]["scaling"];
        let int = |v: &Value| v.as_i64().unwrap_or(0) as i32;
        let (w, h) = (int(&s["width"]), int(&s["height"]));
        match s["type"].as_str() {
            Some("tile") => Scaling::Tile { w, h },
            Some("nine_slice") => {
                let b = &s["border"];
                let border = match b.as_i64() {
                    Some(n) => [n as i32; 4],
                    None => [int(&b["left"]), int(&b["top"]), int(&b["right"]), int(&b["bottom"])],
                };
                Scaling::NineSlice { w, h, border, stretch_inner: s["stretch_inner"].as_bool().unwrap_or(false) }
            }
            _ => Scaling::Stretch,
        }
    }
}

/// One `innerBlit` (`tile: None`) or `innerTiledBlit`: GUI rect `[x0, y0, x1, y1]` and the part
/// `[u0, u1, v0, v1]` of the sprite as fractions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prim {
    pub rect: [i32; 4],
    pub uv: [f32; 4],
    pub tile: Option<[i32; 2]>,
}

/// `blitSprite(pipeline, id, x, y, w, h, color)` once the sprite's scaling is known.
pub fn sprite(scaling: Scaling, x: i32, y: i32, w: i32, h: i32, out: &mut Vec<Prim>) {
    match scaling {
        Scaling::Stretch => {
            if w != 0 && h != 0 {
                out.push(Prim { rect: [x, y, x + w, y + h], uv: [0.0, 1.0, 0.0, 1.0], tile: None });
            }
        }
        Scaling::Tile { w: tw, h: th } => tiled(x, y, w, h, [0, 0, tw, th], [tw, th], out),
        Scaling::NineSlice { w: nw, h: nh, border, stretch_inner } => nine(nw, nh, border, stretch_inner, x, y, w, h, out),
    }
}

/// The private `blitSprite` with a region: `w × h` of a `size` sprite from texel `uv`.
pub fn region(size: [i32; 2], uv: [i32; 2], x: i32, y: i32, w: i32, h: i32, out: &mut Vec<Prim>) {
    if w != 0 && h != 0 {
        let (sw, sh) = (size[0] as f32, size[1] as f32);
        let u = [uv[0] as f32 / sw, (uv[0] + w) as f32 / sw, uv[1] as f32 / sh, (uv[1] + h) as f32 / sh];
        out.push(Prim { rect: [x, y, x + w, y + h], uv: u, tile: None });
    }
}

/// `blitTiledSprite`: `part` is `[u, v, tileW, tileH]` texels of a `size` sprite.
fn tiled(x: i32, y: i32, w: i32, h: i32, part: [i32; 4], size: [i32; 2], out: &mut Vec<Prim>) {
    let [u, v, tw, th] = part;
    if w > 0 && h > 0 && tw > 0 && th > 0 {
        let (sw, sh) = (size[0] as f32, size[1] as f32);
        let uv = [u as f32 / sw, (u + tw) as f32 / sw, v as f32 / sh, (v + th) as f32 / sh];
        out.push(Prim { rect: [x, y, x + w, y + h], uv, tile: Some([tw, th]) });
    }
}

#[allow(clippy::too_many_arguments)]
fn nine(nw: i32, nh: i32, border: [i32; 4], stretch_inner: bool, x: i32, y: i32, w: i32, h: i32, out: &mut Vec<Prim>) {
    let [bl, bt, br, bb] = border;
    let (bl, br, bt, bb) = (bl.min(w / 2), br.min(w / 2), bt.min(h / 2), bb.min(h / 2));
    let size = [nw, nh];
    let corner = |u: i32, v: i32, x: i32, y: i32, cw: i32, ch: i32, out: &mut Vec<Prim>| region(size, [u, v], x, y, cw, ch, out);
    // blitNineSliceInnerSegment: stretched or tiled from texels [u, v, tw, th].
    let inner = |x: i32, y: i32, iw: i32, ih: i32, part: [i32; 4], out: &mut Vec<Prim>| {
        if iw > 0 && ih > 0 {
            if stretch_inner {
                let [u, v, tw, th] = part;
                let (sw, sh) = (nw as f32, nh as f32);
                out.push(Prim { rect: [x, y, x + iw, y + ih], uv: [u as f32 / sw, (u + tw) as f32 / sw, v as f32 / sh, (v + th) as f32 / sh], tile: None });
            } else {
                tiled(x, y, iw, ih, part, size, out);
            }
        }
    };
    let (mw, mh) = (nw - br - bl, nh - bb - bt);
    if w == nw && h == nh {
        corner(0, 0, x, y, w, h, out);
    } else if h == nh {
        corner(0, 0, x, y, bl, h, out);
        inner(x + bl, y, w - br - bl, h, [bl, 0, mw, nh], out);
        corner(nw - br, 0, x + w - br, y, br, h, out);
    } else if w == nw {
        corner(0, 0, x, y, w, bt, out);
        inner(x, y + bt, w, h - bb - bt, [0, bt, nw, mh], out);
        corner(0, nh - bb, x, y + h - bb, w, bb, out);
    } else {
        corner(0, 0, x, y, bl, bt, out);
        inner(x + bl, y, w - br - bl, bt, [bl, 0, mw, bt], out);
        corner(nw - br, 0, x + w - br, y, br, bt, out);
        corner(0, nh - bb, x, y + h - bb, bl, bb, out);
        inner(x + bl, y + h - bb, w - br - bl, bb, [bl, nh - bb, mw, bb], out);
        corner(nw - br, nh - bb, x + w - br, y + h - bb, br, bb, out);
        inner(x, y + bt, bl, h - bb - bt, [0, bt, bl, mh], out);
        inner(x + bl, y + bt, w - br - bl, h - bb - bt, [bl, bt, mw, mh], out);
        inner(x + w - br, y + bt, br, h - bb - bt, [nw - br, bt, br, mh], out);
    }
}

/// `TiledBlitRenderState.buildVertices`: the tiles of a tiled prim, the last of each row and
/// column cut short with its UV lerped.
pub fn tiles(p: &Prim) -> Vec<([i32; 4], [f32; 4])> {
    let Some([tw, th]) = p.tile else { return vec![(p.rect, p.uv)] };
    let [x0, y0, x1, y1] = p.rect;
    let [u0, u1, v0, v1] = p.uv;
    let lerp = |t: f32, a: f32, b: f32| a + t * (b - a);
    let mut out = Vec::new();
    for tx in (0..x1 - x0).step_by(tw as usize) {
        let left = x1 - x0 - tx;
        let (cw, cu) = if tw <= left { (tw, u1) } else { (left, lerp(left as f32 / tw as f32, u0, u1)) };
        for ty in (0..y1 - y0).step_by(th as usize) {
            let left = y1 - y0 - ty;
            let (ch, cv) = if th <= left { (th, v1) } else { (left, lerp(left as f32 / th as f32, v0, v1)) };
            out.push(([x0 + tx, y0 + ty, x0 + tx + cw, y0 + ty + ch], [u0, cu, v0, cv]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_button_slices_into_nine_and_tiles_its_middle() {
        let s = Scaling::from_mcmeta(&json!({"gui": {"scaling": {"type": "nine_slice", "width": 200, "height": 20, "border": 3}}}));
        assert_eq!(s, Scaling::NineSlice { w: 200, h: 20, border: [3; 4], stretch_inner: false });
        let mut out = Vec::new();
        sprite(s, 10, 10, 150, 40, &mut out);
        assert_eq!(out.len(), 9);
        assert_eq!(out[2].rect, [157, 10, 160, 13], "top right corner");
        assert_eq!(out[2].uv, [197.0 / 200.0, 1.0, 0.0, 3.0 / 20.0]);
        assert_eq!(out[7].tile, Some([194, 14]), "the centre tiles the sprite's own middle");
        let mut out = Vec::new();
        sprite(s, 0, 0, 150, 20, &mut out);
        assert_eq!(out.len(), 3, "at the sprite's height only the sides are cut");
    }

    #[test]
    fn the_last_tile_is_cut_with_its_uv() {
        let p = Prim { rect: [0, 0, 25, 10], uv: [0.0, 0.5, 0.0, 1.0], tile: Some([10, 10]) };
        let t = tiles(&p);
        assert_eq!(t.len(), 3);
        assert_eq!(t[2], ([20, 0, 25, 10], [0.0, 0.25, 0.0, 1.0]));
    }
}
