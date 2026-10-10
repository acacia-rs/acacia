//! A book's page (Java's `BookViewScreen` layout: a 192-pixel page area at the top, 14 lines of
//! 114 pixels, the page number above them, the turn buttons under them and Done below). The page
//! is flat parchment: neither look's book art is drawn.

use crate::draw::DrawList;
use crate::theme::Theme;
use crate::widget::{State, Widget, contains};

const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
const AREA: f32 = 192.0;
const TOP: f32 = 2.0;
/// The parchment within the 192-pixel area.
const PAGE: [f32; 4] = [20.0, 1.0, 166.0, 181.0];
const PAPER: [u8; 4] = [0xFD, 0xF5, 0xDE, 0xFF];
const PAPER_EDGE: [u8; 4] = [0x8A, 0x6A, 0x3C, 0xFF];
const TEXT_AT: [f32; 2] = [36.0, 30.0];
pub const TEXT_WIDTH: f32 = 114.0;
pub const LINES: usize = 14;
const MAX_PAGES: usize = 50;
const LINE_HEIGHT: f32 = 9.0;
const NUMBER_AT: [f32; 2] = [148.0, 16.0];
const TURN: [f32; 2] = [23.0, 14.0];
const BACK_AT: [f32; 2] = [43.0, 157.0];
const FORWARD_AT: [f32; 2] = [116.0, 157.0];
const DONE: [f32; 2] = [200.0, 20.0];
const DONE_Y: f32 = 194.0;

/// The page shown: its text, and where it is in the book (`page` counts from 0).
#[derive(Debug, Clone, Copy)]
pub struct Page<'a> {
    pub text: &'a str,
    pub page: usize,
    pub pages: usize,
    /// A book and quill: the text ends in a caret, and the last page turns to a new one.
    pub writing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Back,
    Forward,
    Done,
}

fn rects(size: [f32; 2]) -> [(Hit, [f32; 4]); 3] {
    let left = ((size[0] - AREA) / 2.0).floor();
    let turn = |at: [f32; 2]| [left + at[0], TOP + at[1], left + at[0] + TURN[0], TOP + at[1] + TURN[1]];
    let done_x = ((size[0] - DONE[0]) / 2.0).floor();
    let done_y = (TOP + DONE_Y).min(size[1] - DONE[1] - 2.0);
    [(Hit::Back, turn(BACK_AT)), (Hit::Forward, turn(FORWARD_AT)), (Hit::Done, [done_x, done_y, done_x + DONE[0], done_y + DONE[1]])]
}

fn shows(page: Page, hit: Hit) -> bool {
    match hit {
        Hit::Back => page.page > 0,
        Hit::Forward => page.page + 1 < page.pages || page.writing && page.pages < MAX_PAGES,
        Hit::Done => true,
    }
}

/// The button under `at`; a turn button only where there is a page to turn to.
pub fn hit(page: Page, size: [f32; 2], at: [f32; 2]) -> Option<Hit> {
    rects(size).into_iter().find(|(hit, rect)| shows(page, *hit) && contains(*rect, at)).map(|(hit, _)| hit)
}

/// Whether `text` fits on a page.
pub fn fits(theme: &Theme, text: &str) -> bool {
    theme.font.as_ref().is_none_or(|font| text.split('\n').map(|line| font.wrap(line, TEXT_WIDTH).len().max(1)).sum::<usize>() <= LINES)
}

pub fn draw(list: &mut DrawList, theme: &Theme, page: Page, mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    list.fill(white, [0.0, 0.0, size[0], size[1]], SHADE);
    let left = ((size[0] - AREA) / 2.0).floor();
    let paper = [left + PAGE[0], TOP + PAGE[1], left + PAGE[2], TOP + PAGE[3]];
    list.fill(white, paper, PAPER_EDGE);
    list.fill(white, [paper[0] + 1.0, paper[1] + 1.0, paper[2] - 1.0, paper[3] - 1.0], PAPER);
    let over = hit(page, size, mouse);
    for (button, rect) in rects(size) {
        if !shows(page, button) {
            continue;
        }
        let text = match button {
            Hit::Back => "<",
            Hit::Forward => ">",
            Hit::Done => "Done",
        };
        let state = State { hover: over == Some(button), ..State::default() };
        theme.widgets.skin().draw(list, theme.font.as_ref(), &Widget::Button { text: text.into(), image: None }, rect, state);
    }
    let Some(font) = &theme.font else { return };
    let number = format!("Page {} of {}", page.page + 1, page.pages.max(1));
    font.draw(list, &number, left + NUMBER_AT[0] - font.width(&number), TOP + NUMBER_AT[1], 0x000000, 1.0, false);
    let caret = if page.writing { "_" } else { "" };
    let lines = page.text.split('\n').flat_map(|line| if line.is_empty() { vec![String::new()] } else { font.wrap(line, TEXT_WIDTH) });
    let lines: Vec<String> = lines.take(LINES).collect();
    for (i, line) in lines.iter().enumerate() {
        let end = if i + 1 == lines.len() { caret } else { "" };
        font.draw(list, &format!("{line}{end}"), left + TEXT_AT[0], TOP + TEXT_AT[1] + i as f32 * LINE_HEIGHT, 0x000000, 1.0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_buttons_show_only_where_a_page_follows() {
        let size = [426.0, 240.0];
        let (back, forward, done) = ([165.0, 165.0], [240.0, 165.0], [213.0, 200.0]);
        let first = Page { text: "", page: 0, pages: 3, writing: false };
        assert_eq!((hit(first, size, back), hit(first, size, forward), hit(first, size, done)), (None, Some(Hit::Forward), Some(Hit::Done)));
        let last = Page { page: 2, ..first };
        assert_eq!((hit(last, size, back), hit(last, size, forward)), (Some(Hit::Back), None));
        assert_eq!(hit(Page { writing: true, ..last }, size, forward), Some(Hit::Forward), "a new page");
    }
}
