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
}

pub const WHITE: [u8; 4] = [255; 4];

/// Quads in GUI pixels, scaled to the window as they are added.
#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub quads: Vec<Quad>,
    /// Window pixels per GUI pixel.
    pub scale: f32,
}

impl DrawList {
    pub fn new(scale: f32) -> DrawList {
        DrawList { quads: Vec::new(), scale }
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

    fn quad(&mut self, rect: [f32; 4], uv: [f32; 4], color: [u8; 4]) {
        let s = self.scale;
        self.quads.push(Quad { rect: rect.map(|v| v * s), uv, color });
    }
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
}
