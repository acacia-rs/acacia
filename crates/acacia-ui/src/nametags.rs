//! Name tags over players and named mobs, drawn on the screen where the caller projected them, at a
//! size that shrinks with distance, white over Java's translucent black.

use crate::draw::DrawList;
use crate::font::LINE_HEIGHT;
use crate::theme::Theme;

const BACKGROUND: [u8; 4] = [0, 0, 0, 0x40];

/// One tag: its text and where its bottom centre falls on the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub text: String,
    /// GUI pixels.
    pub x: f32,
    pub y: f32,
    /// GUI pixels per font pixel.
    pub scale: f32,
}

pub fn draw(list: &mut DrawList, theme: &Theme, tags: &[Tag]) {
    let Some(font) = &theme.font else { return };
    let white = theme.atlas.white();
    let base = list.scale;
    for tag in tags {
        list.scale = base * tag.scale;
        let (w, x, y) = (font.width(&tag.text), tag.x / tag.scale, tag.y / tag.scale - LINE_HEIGHT);
        let left = x - w / 2.0;
        list.fill(white, [left - 1.0, y - 1.0, left + w + 1.0, y + LINE_HEIGHT - 1.0], BACKGROUND);
        font.draw(list, &tag.text, left, y, 0xFFFFFF, 1.0, false);
    }
    list.scale = base;
}
