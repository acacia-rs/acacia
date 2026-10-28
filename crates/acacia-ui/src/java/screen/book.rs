//! `BookViewScreen`: the book texture, the page's text wrapped to 114 pixels, the page indicator,
//! the page arrows (`PageButton`) and Done. Shared with `book_edit`.

use super::{Env, Screen, arena_children, render_all, transparent_background};
use crate::java::text::{Style, Text};
use crate::java::widget::button::Button;
use crate::java::widget::input::{Action, Child, Fired, Interaction, Key, KeyOutcome};
use crate::java::widget::layout::Arena;
use crate::java::widget::measure::split;
use crate::java::widget::{Base, Ctx, Widget};

pub const NEXT: &str = "book.page_button.next";
pub const PREVIOUS: &str = "book.page_button.previous";
pub const DONE: &str = "gui.done";

/// `PageButton`: the 23x13 arrow, highlighted while hovered or focused.
pub struct PageButton {
    pub base: Base,
    forward: bool,
}

impl PageButton {
    pub fn new(ctx: &Ctx, x: i32, y: i32, forward: bool) -> PageButton {
        let key = if forward { NEXT } else { PREVIOUS };
        PageButton { base: Base::new(x, y, 23, 13, ctx.tr(key)).with_id(key), forward }
    }
}

impl Widget for PageButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let lit = hovered || self.base.focused;
        let sprite = match (self.forward, lit) {
            (true, false) => "widget/page_forward",
            (true, true) => "widget/page_forward_highlighted",
            (false, false) => "widget/page_backward",
            (false, true) => "widget/page_backward_highlighted",
        };
        ctx.gui.sprite(sprite, self.base.x, self.base.y, 23, 13);
    }
    fn on_click(&mut self, _: f64, _: bool) -> Option<Action> {
        Some(Action::Press)
    }
    fn key_pressed(&mut self, key: Key, _: bool) -> KeyOutcome {
        (self.base.is_active() && key.is_selection()).then_some(Some(Action::Press))
    }
}

/// `backgroundLeft`.
pub fn left(env: &Env) -> i32 {
    (env.width - 192) / 2
}

/// `BOOK_LOCATION` at (left, 2) after the in-game dim.
pub fn background(env: &Env, ctx: &mut Ctx) {
    transparent_background(env, ctx);
    ctx.blit("gui/book", left(env), 2, [0.0, 0.0], 192, 192, [256, 256]);
}

/// `book.pageIndicator` in black without a shadow (`PAGE_TEXT_STYLE`).
pub fn page_indicator(ctx: &Ctx, page: usize, count: usize) -> Text {
    let n = |v: usize| Text::literal(v.to_string());
    ctx.tr_args("book.pageIndicator", &[n(page + 1), n(count)]).with_style(&page_style())
}

pub fn page_style() -> Style {
    Style { color: Some(0), shadow_color: Some(0), ..Style::default() }
}

/// The indicator right-aligned on `left + 148`.
pub fn draw_indicator(ctx: &mut Ctx, env: &Env, indicator: Text) {
    let x = left(env) + 148 - ctx.width(&indicator);
    ctx.label(indicator, x, 2 + 16, 1.0);
}

/// What the screen shows: the pages and the one open.
#[derive(Clone, Debug, Default)]
pub struct BookView {
    pub pages: Vec<Text>,
    pub page: usize,
}

impl BookView {
    /// `setPage`: clamped to the pages.
    pub fn set_page(&mut self, page: usize) {
        self.page = page.min(self.pages.len().saturating_sub(1));
    }

    /// What a fired button or Page Up/Down does to the page; false for anything else.
    pub fn fire(&mut self, id: &str) -> bool {
        match id {
            PREVIOUS => self.page = self.page.saturating_sub(1),
            NEXT if self.page + 1 < self.pages.len() => self.page += 1,
            NEXT => {}
            _ => return false,
        }
        true
    }
}

pub struct BookViewScreen {
    pub env: Env,
    lines: Vec<Text>,
    indicator: Text,
    widgets: Arena,
}

impl BookViewScreen {
    /// `init()` on `book`: Done, then the forward and back arrows (each only where it leads somewhere).
    pub fn new(ctx: &Ctx, env: Env, book: &BookView) -> BookViewScreen {
        let mut done = Button::named(ctx, 200, DONE);
        (done.base.x, done.base.y) = ((env.width - 200) / 2, 2 + 192 + 2);
        let l = left(&env);
        let mut forward = PageButton::new(ctx, l + 116, 2 + 157, true);
        forward.base.visible = book.page + 1 < book.pages.len();
        let mut back = PageButton::new(ctx, l + 43, 2 + 157, false);
        back.base.visible = book.page > 0;
        let page = book.pages.get(book.page).cloned().unwrap_or_default().with_style(&page_style());
        let lines = split(ctx.font, &page, 114);
        let indicator = page_indicator(ctx, book.page, book.pages.len().max(1));
        BookViewScreen { env, lines, indicator, widgets: vec![Box::new(done), Box::new(forward), Box::new(back)] }
    }
}

impl Screen for BookViewScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        render_all(&mut self.widgets, ctx);
        draw_indicator(ctx, &self.env, self.indicator.clone());
        let l = left(&self.env);
        for (i, line) in self.lines.iter().take(128 / 9).enumerate() {
            ctx.label(line.clone(), l + 36, 2 + 30 + i as i32 * 9, 1.0);
        }
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        arena_children(&mut self.widgets)
    }

    /// Page Up and Page Down turn the page (after the focused button had its say).
    fn key_pressed(&mut self, s: &mut Interaction, key: Key, shift: bool) -> Option<Fired> {
        let fired = crate::java::widget::input::key_pressed(&mut self.children(), s, key, shift);
        let page = |id: &'static str| Some(Fired { id: id.into(), action: Action::Press });
        fired.or_else(|| match key {
            Key::PageUp => page(PREVIOUS),
            Key::PageDown => page(NEXT),
            _ => None,
        })
    }
}
