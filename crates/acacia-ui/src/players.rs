//! The player list (Tab), as Java's `PlayerTabOverlay` lays it out: names in up to 20 rows per
//! column, the columns side by side and centred, each row over a translucent background.

use crate::draw::DrawList;
use crate::font::LINE_HEIGHT;
use crate::theme::Theme;

const ROWS: usize = 20;
const TOP: f32 = 10.0;
/// Pixels between a name and the next column.
const GAP: f32 = 5.0;
const BACKGROUND: [u8; 4] = [0, 0, 0, 0x80];
const ROW: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x20];

pub fn draw(list: &mut DrawList, theme: &Theme, names: &[String], width: f32) {
    let Some(font) = &theme.font else { return };
    if names.is_empty() {
        return;
    }
    let white = theme.atlas.white();
    let columns = names.len().div_ceil(ROWS);
    let rows = names.len().div_ceil(columns);
    let column = names.iter().map(|n| font.width(n)).fold(0.0, f32::max) + GAP;
    let total = column * columns as f32;
    let left = ((width - total) / 2.0).floor();
    list.fill(white, [left - 1.0, TOP - 1.0, left + total, TOP + rows as f32 * LINE_HEIGHT], BACKGROUND);
    for (i, name) in names.iter().enumerate() {
        let (x, y) = (left + (i / rows) as f32 * column, TOP + (i % rows) as f32 * LINE_HEIGHT);
        list.fill(white, [x, y, x + column - 1.0, y + 8.0], ROW);
        font.draw(list, name, x, y, 0xFFFFFF, 1.0, true);
    }
}
