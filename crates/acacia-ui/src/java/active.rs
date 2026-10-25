//! Finding the style under the mouse in drawn text (`ActiveTextCollector.findElementUnderCursor`):
//! the text's bounds through its pose must hold the mouse, then every glyph's active area (and
//! each empty glyph's: spaces) holding the mouse, in pose-local coordinates, is a hit, in
//! `Font.GlyphVisitor` order (glyphs, then empty areas).

use super::gui::Pose;
use super::mth::{ceil, floor};
use super::text::Style;

/// A glyph's `ActiveArea`: left, top, right, bottom before the pose.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveArea {
    pub rect: [f32; 4],
    pub style: Style,
}

/// `Font.prepareText(..., includeEmpty = true)` as far as hovering reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextAreas {
    /// `PreparedText.bounds` as floats; `None` when nothing is drawn.
    pub bounds: Option<[f32; 4]>,
    pub areas: Vec<ActiveArea>,
}

/// `Matrix3x2f.invert` as JOML computes it.
pub fn invert(m: &Pose) -> Pose {
    let s = 1.0 / (m[0] * m[3] - m[1] * m[2]);
    [m[3] * s, -m[1] * s, -m[2] * s, m[0] * s, (m[2] * m[5] - m[4] * m[3]) * s, (m[4] * m[1] - m[0] * m[5]) * s]
}

/// `ScreenRectangle.transformMaxBounds` of `PreparedText.bounds()`: left, top, width, height.
fn screen_bounds(b: [f32; 4], m: &Pose) -> [i32; 4] {
    let (l, t) = (floor(b[0]), floor(b[1]));
    let (r, bt) = (ceil(b[2]), ceil(b[3]));
    let corners = [(l, t), (r, t), (l, bt), (r, bt)].map(|(x, y)| super::gui::transform(m, x as f32, y as f32));
    let min_x = corners.iter().map(|c| c[0]).fold(f32::MAX, f32::min);
    let max_x = corners.iter().map(|c| c[0]).fold(-f32::MAX, f32::max);
    let min_y = corners.iter().map(|c| c[1]).fold(f32::MAX, f32::min);
    let max_y = corners.iter().map(|c| c[1]).fold(-f32::MAX, f32::max);
    [floor(min_x), floor(min_y), ceil(max_x - min_x), ceil(max_y - min_y)]
}

/// The styles hit at `mouse` (GUI pixels), in visit order.
pub fn styles_under<'a>(t: &'a TextAreas, pose: &Pose, mouse: (i32, i32)) -> Vec<&'a Style> {
    let Some(bounds) = t.bounds else { return Vec::new() };
    let [x, y, w, h] = screen_bounds(bounds, pose);
    let (mx, my) = mouse;
    if !(mx >= x && mx < x + w && my >= y && my < y + h) {
        return Vec::new();
    }
    let [lx, ly] = super::gui::transform(&invert(pose), mx as f32, my as f32);
    t.areas.iter().filter(|a| lx >= a.rect[0] && lx < a.rect[2] && ly >= a.rect[1] && ly < a.rect[3]).map(|a| &a.style).collect()
}

/// `ActiveTextCollector.isPointInRectangle` with the mouse taken through `pose`'s inverse.
pub fn mouse_over(pose: &Pose, mouse: (i32, i32), rect: [f32; 4]) -> bool {
    let [x, y] = super::gui::transform(&invert(pose), mouse.0 as f32, mouse.1 as f32);
    x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]
}
