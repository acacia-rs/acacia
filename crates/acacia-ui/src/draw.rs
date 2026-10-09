//! What the UI hands the renderer each frame: textured, tinted rectangles in screen pixels, drawn
//! in order over the world.

use crate::atlas::Sprite;

/// One rectangle. `uv` is in atlas texels, so sprites keep exact pixels at any GUI scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// Left, top, right, bottom in window pixels.
    pub rect: [f32; 4],
    /// Left, top, right, bottom in atlas texels.
    pub uv: [f32; 4],
    /// Multiplies the texel, straight alpha.
    pub color: [u8; 4],
    /// An enchanted item's icon: the renderer shimmers its opaque texels.
    pub glint: bool,
}

pub const WHITE: [u8; 4] = [255; 4];

/// Quads in GUI pixels, scaled to the window as they are added.
#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub quads: Vec<Quad>,
    /// Window pixels per GUI pixel.
    pub scale: f32,
    /// Quads added are cut to this rectangle (GUI pixels), UVs with them.
    pub clip: Option<[f32; 4]>,
}

impl DrawList {
    pub fn new(scale: f32) -> DrawList {
        DrawList { quads: Vec::new(), scale, clip: None }
    }

    /// `sprite` at its own size with its top-left at (`x`, `y`) GUI pixels.
    pub fn sprite(&mut self, sprite: Sprite, x: f32, y: f32, color: [u8; 4]) {
        self.sprite_part(sprite, x, y, [0.0, 0.0, sprite.width as f32, sprite.height as f32], color);
    }

    /// The part `[left, top, right, bottom]` (texels within the sprite) of `sprite`, placed at
    /// (`x`, `y`) GUI pixels: a partly filled bar, half a heart.
    pub fn sprite_part(&mut self, sprite: Sprite, x: f32, y: f32, part: [f32; 4], color: [u8; 4]) {
        let [l, t, r, b] = part;
        let (u, v) = (sprite.x as f32, sprite.y as f32);
        self.quad([x, y, x + r - l, y + b - t], [u + l, v + t, u + r, v + b], color);
    }

    /// `sprite` stretched over `rect` (GUI pixels).
    pub fn sprite_stretched(&mut self, sprite: Sprite, rect: [f32; 4], color: [u8; 4]) {
        let (u, v) = (sprite.x as f32, sprite.y as f32);
        self.quad(rect, [u, v, u + sprite.width as f32, v + sprite.height as f32], color);
    }

    /// A flat colour over `rect` (GUI pixels); `white` is an opaque white texel in the atlas.
    pub fn fill(&mut self, white: Sprite, rect: [f32; 4], color: [u8; 4]) {
        let (u, v) = (white.x as f32 + 0.5, white.y as f32 + 0.5);
        self.quad(rect, [u, v, u, v], color);
    }

    /// An item's icon stretched over `rect` (GUI pixels), shimmering when `glint`.
    pub fn icon(&mut self, sprite: Sprite, rect: [f32; 4], glint: bool) {
        let (u, v) = (sprite.x as f32, sprite.y as f32);
        self.push(rect, [u, v, u + sprite.width as f32, v + sprite.height as f32], WHITE, glint);
    }

    pub(crate) fn quad(&mut self, rect: [f32; 4], uv: [f32; 4], color: [u8; 4]) {
        self.push(rect, uv, color, false);
    }

    fn push(&mut self, rect: [f32; 4], uv: [f32; 4], color: [u8; 4], glint: bool) {
        let Some((cut, uv)) = clipped(rect, uv, self.clip) else { return };
        let s = self.scale;
        // A cut icon does not shimmer: the renderer lays the glint over a whole quad.
        self.quads.push(Quad { rect: cut.map(|v| v * s), uv, color, glint: glint && cut == rect });
    }
}

/// `rect` cut to `clip`, its UVs cut in proportion; `None` when nothing is left.
fn clipped(rect: [f32; 4], uv: [f32; 4], clip: Option<[f32; 4]>) -> Option<([f32; 4], [f32; 4])> {
    let Some([cl, ct, cr, cb]) = clip else { return Some((rect, uv)) };
    let [l, t, r, b] = rect;
    let (nl, nt, nr, nb) = (l.max(cl), t.max(ct), r.min(cr), b.min(cb));
    if nl >= nr || nt >= nb {
        return None;
    }
    let lerp = |a: f32, b: f32, from: f32, to: f32, at: f32| if to == from { a } else { a + (b - a) * (at - from) / (to - from) };
    let [ul, ut, ur, ub] = uv;
    Some(([nl, nt, nr, nb], [lerp(ul, ur, l, r, nl), lerp(ut, ub, t, b, nt), lerp(ul, ur, l, r, nr), lerp(ut, ub, t, b, nb)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_scale_with_the_gui() {
        let sprite = Sprite { x: 10, y: 20, width: 9, height: 9 };
        let mut list = DrawList::new(2.0);
        list.sprite_part(sprite, 5.0, 6.0, [0.0, 0.0, 5.0, 9.0], WHITE);
        assert_eq!(list.quads[0].rect, [10.0, 12.0, 20.0, 30.0]);
        assert_eq!(list.quads[0].uv, [10.0, 20.0, 15.0, 29.0]);
    }

    #[test]
    fn only_a_whole_enchanted_icon_shimmers() {
        let sprite = Sprite { x: 0, y: 0, width: 16, height: 16 };
        let mut list = DrawList::new(2.0);
        list.icon(sprite, [0.0, 0.0, 16.0, 16.0], false);
        list.icon(sprite, [20.0, 0.0, 36.0, 16.0], true);
        list.clip = Some([0.0, 0.0, 48.0, 16.0]);
        list.icon(sprite, [40.0, 0.0, 56.0, 16.0], true);
        list.icon(sprite, [60.0, 0.0, 76.0, 16.0], true);
        assert_eq!(list.quads.iter().map(|q| q.glint).collect::<Vec<_>>(), [false, true, false], "the last is clipped away");
    }

    #[test]
    fn clipping_cuts_uvs_in_proportion() {
        let sprite = Sprite { x: 0, y: 0, width: 10, height: 10 };
        let mut list = DrawList::new(1.0);
        list.clip = Some([5.0, 0.0, 100.0, 100.0]);
        list.sprite_stretched(sprite, [0.0, 0.0, 20.0, 20.0], WHITE);
        list.sprite(sprite, -20.0, 0.0, WHITE);
        assert_eq!(list.quads.len(), 1, "wholly outside is dropped");
        assert_eq!(list.quads[0].rect, [5.0, 0.0, 20.0, 20.0]);
        assert_eq!(list.quads[0].uv, [2.5, 0.0, 10.0, 10.0]);
    }
}
