//! Bedrock's button menus (pause, options): the pause screen's left column, light buttons 28 tall
//! and 3 apart under the game's logo, over a dimmed left band
//! (research/pause-bedrock-layout.md in the workspace). The Java look draws Java's own screens
//! (`java::screen`).

use crate::draw::DrawList;
use crate::theme::Theme;
use crate::widget::{State, Widget, contains};

/// The logo's atlas name (Bedrock's `textures/ui/title`).
pub const LOGO: &str = "ui/title";

const HEIGHT: f32 = 28.0;
const GAP: f32 = 3.0;
/// The column: `0.3729·W − 10` wide, centred at `0.294·W`; the dim band ends at `0.5565·W`.
const WIDTH: f32 = 0.3729;
const CENTRE: f32 = 0.294;
const BAND: f32 = 0.5565;
/// Gap under the logo, which is a fifth as tall as the column is wide.
const LOGO_GAP: f32 = 4.0;

/// Where the heading goes and each button's rect, for a screen `size` GUI pixels big. An empty
/// title shows the logo instead.
fn layout(theme: &Theme, title: &str, count: usize, [w, h]: [f32; 2]) -> ([f32; 4], Vec<[f32; 4]>) {
    let n = count as f32;
    let width = (WIDTH * w - 10.0).floor();
    let x = (CENTRE * w - width / 2.0).floor();
    let heading = if title.is_empty() && theme.atlas.get(LOGO).is_some() { (width * 0.2).floor() } else { crate::font::LINE_HEIGHT };
    let step = HEIGHT + GAP;
    let rows = if n * step > h { (count / 2 + 1) as f32 } else { n };
    let total = heading + LOGO_GAP + rows * HEIGHT + (rows - 1.0).max(0.0) * GAP;
    let top = ((h - total) / 2.0).floor();
    let first = top + heading + LOGO_GAP;
    let head = [x, top, x + width, top + heading];
    if rows < n {
        return (head, two_columns(count, [x, x + width + GAP], width, [x, width], first, step));
    }
    (head, (0..count).map(|i| [x, first + i as f32 * step, x + width, first + i as f32 * step + HEIGHT]).collect())
}

/// Buttons too many for one column: all but the last in two `columns` (their left edges), row by
/// row, and the last (Done) alone below at `last` (left edge and width).
fn two_columns(count: usize, columns: [f32; 2], width: f32, last: [f32; 2], top: f32, step: f32) -> Vec<[f32; 4]> {
    let paired = count.saturating_sub(1);
    let cell = |i: usize| {
        let (x, y) = (columns[i % 2], top + (i / 2) as f32 * step);
        [x, y, x + width, y + HEIGHT]
    };
    let below = top + paired.div_ceil(2) as f32 * step;
    (0..paired).map(cell).chain((count > 0).then_some([last[0], below, last[0] + last[1], below + HEIGHT])).collect()
}

/// The button under `mouse`, if any.
pub fn hit(theme: &Theme, title: &str, count: usize, size: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    layout(theme, title, count, size).1.iter().position(|&r| contains(r, mouse))
}

pub fn draw(list: &mut DrawList, theme: &Theme, title: &str, buttons: &[String], mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    let font = theme.font.as_ref();
    let (heading, rects) = layout(theme, title, buttons.len(), size);
    list.fill(white, [0.0, 0.0, size[0], size[1]], [0, 0, 0, 26]);
    list.fill(white, [0.0, 0.0, (BAND * size[0]).floor(), size[1]], [0, 0, 0, 128]);
    match theme.atlas.get(LOGO).filter(|_| title.is_empty()) {
        Some(logo) => list.sprite_stretched(logo, heading, crate::draw::WHITE),
        None => {
            if let Some(font) = font {
                let x = ((heading[0] + heading[2]) / 2.0 - font.width(title) / 2.0).floor();
                font.draw(list, title, x, heading[1], 0xFFFFFF, 1.0, true);
            }
        }
    }
    let skin = theme.widgets.skin();
    for (label, rect) in buttons.iter().zip(rects) {
        let hover = contains(rect, mouse);
        skin.draw(list, font, &Widget::Button { text: label.clone(), image: None }, rect, State { hover, ..State::default() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::test_theme::theme;

    #[test]
    fn a_long_menu_takes_two_columns_and_fits() {
        let (theme, size) = (theme(false), [427.0, 240.0]);
        let rects = layout(&theme, "Options", 9, size).1;
        assert_eq!(rects.len(), 9);
        assert!(rects.iter().all(|r| r[0] >= 0.0 && r[2] <= size[0] && r[3] <= size[1]), "{rects:?}");
        // Pairs share a row; Done sits alone under them.
        assert!(rects[0][1] == rects[1][1] && rects[0][2] <= rects[1][0] && rects[8][1] > rects[7][1], "{rects:?}");
    }

    #[test]
    fn the_column_sits_left_of_centre() {
        let (theme, size) = (theme(false), [376.0, 250.0]);
        let (_, rects) = layout(&theme, "Options", 3, size);
        // 0.3729·376 − 10 = 130 wide, centred at 110.
        assert_eq!(rects[0][0], 45.0);
        assert_eq!(rects[0][2] - rects[0][0], 130.0);
        assert_eq!(rects[1][1] - rects[0][1], 31.0, "28 tall, 3 apart");
        assert_eq!(hit(&theme, "Options", 3, size, [46.0, rects[2][1] + 1.0]), Some(2));
    }
}
