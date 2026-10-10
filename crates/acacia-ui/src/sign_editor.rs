//! The sign editor (Java's `AbstractSignEditScreen`): the title, the board with its four lines,
//! the line being typed between `>` and `<`, and a Done button. A hanging sign's board is smaller,
//! on two chains, and takes its narrower lines.

use crate::draw::DrawList;
use crate::signs::{HANGING, Metrics, SIGN};
use crate::theme::Theme;
use crate::widget::{State, TextEdit, Widget, contains};

pub const LINES: usize = 4;
/// Java's `Screen` background gradient, flattened.
const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
/// The board: a sign's 24×12 face at four GUI pixels a texel, in oak's colours.
const BOARD: [f32; 2] = [96.0, 48.0];
const BOARD_TOP: f32 = 66.0;
const WOOD: [u8; 4] = [0xA5, 0x85, 0x51, 0xFF];
const WOOD_EDGE: [u8; 4] = [0x6B, 0x51, 0x32, 0xFF];
const POST: [f32; 2] = [8.0, 30.0];
const HANGING_BOARD: [f32; 2] = [72.0, 44.0];
/// A chain's distance from the middle, and its size.
const CHAIN: [f32; 3] = [22.0, 2.0, 14.0];
const TITLE_Y: f32 = 40.0;
const BUTTON: [f32; 2] = [200.0, 20.0];

/// What is being written: the lines and which one the caret is in.
#[derive(Debug, Clone, Copy)]
pub struct Editing<'a> {
    pub lines: &'a [TextEdit; LINES],
    pub line: usize,
    pub hanging: bool,
}

fn metrics(hanging: bool) -> Metrics {
    if hanging { HANGING } else { SIGN }
}

/// The Done button, for a GUI `size` big.
pub fn done_rect(size: [f32; 2]) -> [f32; 4] {
    let (x, y) = (((size[0] - BUTTON[0]) / 2.0).floor(), (size[1] / 4.0 + 144.0).min(size[1] - BUTTON[1] - 4.0).floor());
    [x, y, x + BUTTON[0], y + BUTTON[1]]
}

/// Whether `text` fits on a line of a sign, or of a hanging one.
pub fn fits(theme: &Theme, text: &str, hanging: bool) -> bool {
    theme.font.as_ref().is_none_or(|font| font.width(text) <= metrics(hanging).max_width)
}

pub fn draw(list: &mut DrawList, theme: &Theme, editing: Editing, mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    list.fill(white, [0.0, 0.0, size[0], size[1]], SHADE);
    let middle = (size[0] / 2.0).floor();
    let (metrics, wood) = (metrics(editing.hanging), if editing.hanging { HANGING_BOARD } else { BOARD });
    let board = [middle - wood[0] / 2.0, BOARD_TOP, middle + wood[0] / 2.0, BOARD_TOP + wood[1]];
    if editing.hanging {
        for chain in [middle - CHAIN[0] - CHAIN[1], middle + CHAIN[0]] {
            list.fill(white, [chain, board[1] - CHAIN[2], chain + CHAIN[1], board[1]], WOOD_EDGE);
        }
    } else {
        list.fill(white, [middle - POST[0] / 2.0, board[3], middle + POST[0] / 2.0, board[3] + POST[1]], WOOD_EDGE);
    }
    list.fill(white, board, WOOD_EDGE);
    list.fill(white, [board[0] + 2.0, board[1] + 2.0, board[2] - 2.0, board[3] - 2.0], WOOD);
    let done = done_rect(size);
    let state = State { hover: contains(done, mouse), ..State::default() };
    theme.widgets.skin().draw(list, theme.font.as_ref(), &Widget::Button { text: "Done".into(), image: None }, done, state);
    let Some(font) = &theme.font else { return };
    let title = "Edit Sign Message";
    font.draw(list, title, middle - (font.width(title) / 2.0).floor(), TITLE_Y, 0xFFFFFF, 1.0, true);
    let top = BOARD_TOP + (wood[1] - LINES as f32 * metrics.line_height) / 2.0 + 1.0;
    for (i, edit) in editing.lines.iter().enumerate() {
        let (text, y) = (edit.text(), top + i as f32 * metrics.line_height);
        let start = middle - (font.width(&text) / 2.0).trunc();
        font.draw(list, &text, start, y, 0x000000, 1.0, false);
        if i == editing.line {
            let end = start + font.width(&text);
            font.draw(list, ">", start - font.width("> "), y, 0x000000, 1.0, false);
            font.draw(list, "<", end + font.width(" "), y, 0x000000, 1.0, false);
            font.draw(list, "_", start + font.width(&edit.before_caret()), y + 1.0, 0x000000, 1.0, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_done_button_sits_under_the_board_on_any_screen() {
        assert_eq!(done_rect([426.0, 240.0]), [113.0, 204.0, 313.0, 224.0]);
        assert_eq!(done_rect([320.0, 180.0])[3], 176.0, "kept on a short screen");
    }
}
