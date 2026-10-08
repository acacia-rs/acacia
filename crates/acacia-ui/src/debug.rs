//! The debug screen (F3): lines of text down the left side and the right side, each over a grey
//! background, as Java's `DebugScreenOverlay` draws them.

use crate::draw::DrawList;
use crate::font::LINE_HEIGHT;
use crate::theme::Theme;

const TEXT: u32 = 0xE0E0E0;
/// Java's `0x90505050`.
const BACKGROUND: [u8; 4] = [0x50, 0x50, 0x50, 0x90];

/// `left` and `right` lines; an empty line leaves a gap.
pub fn draw(list: &mut DrawList, theme: &Theme, left: &[String], right: &[String], width: f32) {
    let Some(font) = &theme.font else { return };
    let white = theme.atlas.white();
    for (side, lines) in [(false, left), (true, right)] {
        for (i, line) in lines.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
            let y = 2.0 + i as f32 * LINE_HEIGHT;
            let w = font.width(line);
            let x = if side { width - 2.0 - w } else { 2.0 };
            list.fill(white, [x - 1.0, y - 1.0, x + w + 1.0, y + LINE_HEIGHT - 1.0], BACKGROUND);
            font.draw(list, line, x, y, TEXT, 1.0, false);
        }
    }
}
