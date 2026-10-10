//! A book's page (Java's `BookViewScreen` layout: a 192-pixel page area at the top, 14 lines of
//! 114 pixels, the page number above them, the turn buttons under them and Done below). A book and
//! quill has Sign beside Done; signing swaps the page for the title prompt of Java's
//! `BookSignScreen`. The page is flat parchment: neither look's book art is drawn.

use crate::draw::DrawList;
use crate::font::Font;
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
/// Two buttons share Done's row, this far apart.
const GAP: f32 = 4.0;
/// The signing prompt's rows down the page: the heading, the title, the warning.
const SIGNING_Y: [f32; 3] = [34.0, 50.0, 82.0];
const HEADING: &str = "Enter Book Title:";
const WARNING: &str = "Note! When you sign the book, it will no longer be editable.";
const GREY: u32 = 0x555555;

/// The page shown: its text, and where it is in the book (`page` counts from 0).
#[derive(Debug, Clone, Copy)]
pub struct Page<'a> {
    pub text: &'a str,
    pub page: usize,
    pub pages: usize,
    /// A book and quill: the text ends in a caret, and the last page turns to a new one.
    pub writing: bool,
    /// The title typed so far, while the book is being signed.
    pub signing: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Back,
    Forward,
    Done,
    /// To the title prompt.
    Sign,
    /// Sign with the title typed.
    Finalize,
    Cancel,
}

/// The buttons `page` shows, with their labels.
fn buttons(page: Page, size: [f32; 2]) -> Vec<(Hit, [f32; 4], &'static str)> {
    let left = ((size[0] - AREA) / 2.0).floor();
    let turn = |at: [f32; 2]| [left + at[0], TOP + at[1], left + at[0] + TURN[0], TOP + at[1] + TURN[1]];
    let (x, y) = (((size[0] - DONE[0]) / 2.0).floor(), (TOP + DONE_Y).min(size[1] - DONE[1] - 2.0));
    let half = (DONE[0] - GAP) / 2.0;
    let (whole, first, second) = ([x, y, x + DONE[0], y + DONE[1]], [x, y, x + half, y + DONE[1]], [x + half + GAP, y, x + DONE[0], y + DONE[1]]);
    if page.signing.is_some() {
        return vec![(Hit::Finalize, first, "Sign and Close"), (Hit::Cancel, second, "Cancel")];
    }
    let mut shown = if page.writing { vec![(Hit::Sign, first, "Sign"), (Hit::Done, second, "Done")] } else { vec![(Hit::Done, whole, "Done")] };
    if page.page > 0 {
        shown.push((Hit::Back, turn(BACK_AT), "<"));
    }
    if page.page + 1 < page.pages || page.writing && page.pages < MAX_PAGES {
        shown.push((Hit::Forward, turn(FORWARD_AT), ">"));
    }
    shown
}

/// The button under `at`; a turn button only where there is a page to turn to.
pub fn hit(page: Page, size: [f32; 2], at: [f32; 2]) -> Option<Hit> {
    buttons(page, size).into_iter().find(|(_, rect, _)| contains(*rect, at)).map(|(hit, ..)| hit)
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
    for (button, rect, text) in buttons(page, size) {
        let state = State { hover: over == Some(button), ..State::default() };
        theme.widgets.skin().draw(list, theme.font.as_ref(), &Widget::Button { text: text.into(), image: None }, rect, state);
    }
    let Some(font) = &theme.font else { return };
    if let Some(title) = page.signing {
        return draw_signing(list, font, title, left);
    }
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

fn draw_signing(list: &mut DrawList, font: &Font, title: &str, left: f32) {
    let middle = left + TEXT_AT[0] + TEXT_WIDTH / 2.0;
    let centred = |list: &mut DrawList, text: &str, y: f32, colour: u32| font.draw(list, text, middle - (font.width(text) / 2.0).floor(), TOP + y, colour, 1.0, false);
    centred(list, HEADING, SIGNING_Y[0], 0x000000);
    centred(list, &format!("{title}_"), SIGNING_Y[1], 0x000000);
    for (i, line) in font.wrap(WARNING, TEXT_WIDTH).iter().enumerate() {
        font.draw(list, line, left + TEXT_AT[0], TOP + SIGNING_Y[2] + i as f32 * LINE_HEIGHT, GREY, 1.0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_buttons_show_only_where_a_page_follows() {
        let size = [426.0, 240.0];
        let (back, forward, done) = ([165.0, 165.0], [240.0, 165.0], [213.0, 200.0]);
        let first = Page { text: "", page: 0, pages: 3, writing: false, signing: None };
        assert_eq!((hit(first, size, back), hit(first, size, forward), hit(first, size, done)), (None, Some(Hit::Forward), Some(Hit::Done)));
        let last = Page { page: 2, ..first };
        assert_eq!((hit(last, size, back), hit(last, size, forward)), (Some(Hit::Back), None));
        assert_eq!(hit(Page { writing: true, ..last }, size, forward), Some(Hit::Forward), "a new page");
    }

    #[test]
    fn a_book_and_quill_signs_from_the_left_button() {
        let size = [426.0, 240.0];
        let (left, right) = ([150.0, 200.0], [280.0, 200.0]);
        let quill = Page { text: "", page: 0, pages: 1, writing: true, signing: None };
        assert_eq!((hit(quill, size, left), hit(quill, size, right)), (Some(Hit::Sign), Some(Hit::Done)));
        let signing = Page { signing: Some("Title"), ..quill };
        assert_eq!((hit(signing, size, left), hit(signing, size, right)), (Some(Hit::Finalize), Some(Hit::Cancel)));
        assert_eq!(hit(signing, size, [240.0, 165.0]), None, "no page turns while signing");
    }
}
