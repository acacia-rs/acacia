//! `BookEditScreen`: a writable book's page in a borderless `MultiLineEditBox` over the book
//! texture, the page arrows (forward adds a page at the end), Sign and Done.

use super::book::{self, DONE, NEXT, PREVIOUS, PageButton, draw_indicator, page_indicator};
use super::{Env, Screen, arena_children, render_all};
use crate::java::gui::Metrics;
use crate::java::widget::button::Button;
use crate::java::widget::input::{Child, Key};
use crate::java::widget::layout::Arena;
use crate::java::widget::multiline_box::MultiLineEditBox;
use crate::java::widget::Ctx;

pub const SIGN: &str = "book.signButton";

/// The book being written: its pages and the page editor, kept by the owner across frames. The
/// editor takes its input here (it needs the font); the buttons through [`BookEditScreen`].
#[derive(Clone, Debug)]
pub struct BookEdit {
    pub pages: Vec<String>,
    pub page: usize,
    pub editor: MultiLineEditBox,
}

impl BookEdit {
    /// `new BookEditScreen(..)` and `init` at `now` (millis): at least one page, the first open
    /// and the editor focused.
    pub fn new(m: &dyn Metrics, mut pages: Vec<String>, now: i64) -> BookEdit {
        if pages.is_empty() {
            pages.push(String::new());
        }
        let mut editor = MultiLineEditBox::new(m, 0, 28, 122, 134);
        editor.area.show_background = false;
        editor.area.show_decorations = false;
        editor.text_color = 0xFF00_0000;
        editor.cursor_color = 0xFF00_0000;
        editor.shadow = false;
        editor.field.char_limit = Some(1024);
        editor.field.line_limit = Some(126 / 9);
        editor.area.base.focused = true;
        editor.focused_time = now;
        let mut book = BookEdit { pages, page: 0, editor };
        book.load_page(m);
        book
    }

    /// `updatePageContent`: the open page into the editor (past its line limit if it must).
    pub fn load_page(&mut self, m: &dyn Metrics) {
        let value = self.pages[self.page].clone();
        self.editor.set_value(m, &value, true);
    }

    fn store(&mut self) {
        self.pages[self.page].clone_from(&self.editor.field.value);
    }

    /// `pageBack` / `pageForward` (which appends a page, up to 100); false for other ids.
    pub fn fire(&mut self, m: &dyn Metrics, id: &str) -> bool {
        match id {
            PREVIOUS => {
                if self.page > 0 {
                    self.page -= 1;
                    self.load_page(m);
                }
            }
            NEXT => {
                if self.page + 1 >= self.pages.len() && self.pages.len() < 100 {
                    self.pages.push(String::new());
                }
                if self.page + 1 < self.pages.len() {
                    self.page += 1;
                }
                self.load_page(m);
            }
            _ => return false,
        }
        true
    }

    /// `charTyped` into the focused editor.
    pub fn char_typed(&mut self, m: &dyn Metrics, c: char) {
        if self.editor.char_typed(m, c) {
            self.store();
        }
    }

    /// `keyPressed`: Page Up and Page Down turn the page, the rest go to the focused editor; false
    /// when unused (the screen's buttons may take it).
    pub fn key(&mut self, m: &dyn Metrics, key: Key, shift: bool) -> bool {
        match key {
            Key::PageUp => self.fire(m, PREVIOUS),
            Key::PageDown => self.fire(m, NEXT),
            _ if self.editor.area.base.focused => {
                let used = self.editor.key(m, key, shift);
                self.store();
                used
            }
            _ => false,
        }
    }

    /// A left click at (`x`, `y`) on a `width`-wide screen: on the editor it moves the cursor and
    /// focuses it (true); elsewhere the editor loses the focus (false: the buttons take it).
    pub fn mouse_clicked(&mut self, m: &dyn Metrics, width: i32, x: f64, y: f64, shift: bool) -> bool {
        self.editor.area.base.x = editor_x(width);
        let on = self.editor.area.over(x, y);
        self.editor.area.base.focused = on;
        if on {
            self.editor.click(m, x, y, shift, false);
        }
        on
    }

    /// The mouse dragged to (`x`, `y`) after clicking the editor.
    pub fn mouse_dragged(&mut self, m: &dyn Metrics, x: f64, y: f64) {
        if self.editor.area.base.focused {
            self.editor.click(m, x, y, false, true);
        }
    }

    /// The pages `saveChanges` sends: empty ones at the end dropped.
    pub fn saved_pages(&self) -> Vec<String> {
        let mut pages = self.pages.clone();
        while pages.last().is_some_and(String::is_empty) {
            pages.pop();
        }
        pages
    }
}

fn editor_x(width: i32) -> i32 {
    (width - 114) / 2 - 8
}

pub struct BookEditScreen {
    pub env: Env,
    indicator: crate::java::text::Text,
    widgets: Arena,
}

impl BookEditScreen {
    /// `init()`: the editor centred over the book, back and forward arrows, Sign and Done.
    pub fn new(ctx: &Ctx, env: Env, book: &BookEdit) -> BookEditScreen {
        let mut editor = book.editor.clone();
        editor.area.base.x = editor_x(env.width);
        let l = book::left(&env);
        let mut back = PageButton::new(ctx, l + 43, 2 + 157, false);
        back.base.visible = book.page > 0;
        let forward = PageButton::new(ctx, l + 116, 2 + 157, true);
        let top = 2 + 192 + 2;
        let mut sign = Button::named(ctx, 98, SIGN);
        (sign.base.x, sign.base.y) = (env.width / 2 - 98 - 2, top);
        let mut done = Button::named(ctx, 98, DONE);
        (done.base.x, done.base.y) = (env.width / 2 + 2, top);
        let widgets: Arena = vec![Box::new(editor), Box::new(back), Box::new(forward), Box::new(sign), Box::new(done)];
        let indicator = page_indicator(ctx, book.page, book.pages.len());
        BookEditScreen { env, indicator, widgets }
    }
}

impl Screen for BookEditScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        book::background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
        draw_indicator(ctx, &self.env, self.indicator.clone());
    }

    /// The buttons; the editor takes its input through [`BookEdit`].
    fn children(&mut self) -> Vec<Child<'_>> {
        arena_children(&mut self.widgets[1..])
    }
}
