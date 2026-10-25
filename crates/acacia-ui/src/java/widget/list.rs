//! `AbstractScrollArea`'s scroll bar and `AbstractSelectionList`: rows stacked under a scroll
//! amount, the list background, the scissor, the selection outline, the separators and the bar.

use super::input::Child;
use super::{Base, Ctx, Widget};

/// An `AbstractSelectionList.Entry`: drawn into its row box.
pub trait Entry {
    /// `extractContent`; `row` is the entry's (x, y, w, h).
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], hovered: bool);
    /// Whether it holds the list's focus (`ContainerObjectSelectionList`: one of its children).
    fn focused(&self) -> bool {
        false
    }
    /// A `ContainerObjectSelectionList.Entry`'s widgets.
    fn children(&mut self) -> Vec<&mut dyn Widget> {
        Vec::new()
    }
    /// Its widgets moved into `row` as `extract_content` places them.
    fn place(&mut self, _row: [i32; 4]) {}
}

pub struct Row {
    pub entry: Box<dyn Entry>,
    pub h: i32,
}

pub struct SelectionList {
    pub base: Base,
    pub rows: Vec<Row>,
    pub scroll: f64,
    pub row_width: i32,
    /// `getSelected`.
    pub selected: Option<usize>,
    /// `getFocused() != null`.
    pub focused: bool,
    /// `entriesCanBeSelected` (false for `ContainerObjectSelectionList`).
    pub selectable: bool,
    /// `minecraft.level != null`: the in-world textures.
    pub in_world: bool,
}

impl SelectionList {
    /// `new AbstractSelectionList(minecraft, width, height, y, entryHeight)`.
    pub fn new(w: i32, h: i32, y: i32, row_width: i32, in_world: bool) -> SelectionList {
        let base = Base::new(0, y, w, h, crate::java::text::Text::default());
        SelectionList { base, rows: Vec::new(), scroll: 0.0, row_width, selected: None, focused: false, selectable: true, in_world }
    }

    pub fn push(&mut self, entry: Box<dyn Entry>, h: i32) {
        self.rows.push(Row { entry, h });
    }

    pub fn row_left(&self) -> i32 {
        self.base.x + self.base.w / 2 - self.row_width / 2
    }

    pub fn row_right(&self) -> i32 {
        self.row_left() + self.row_width
    }

    /// Each row's top, `repositionEntries` from the scroll amount.
    pub fn row_tops(&self) -> Vec<i32> {
        let mut y = self.base.y + 2 - self.scroll as i32;
        self.rows.iter().map(|r| {
            let top = y;
            y += r.h;
            top
        }).collect()
    }

    pub fn content_height(&self) -> i32 {
        self.rows.iter().map(|r| r.h).sum::<i32>() + 4
    }

    pub fn max_scroll(&self) -> i32 {
        (self.content_height() - self.base.h).max(0)
    }

    /// `setScrollAmount`: clamped to what the content allows.
    pub fn set_scroll(&mut self, v: f64) {
        self.scroll = v.clamp(0.0, f64::from(self.max_scroll()));
    }

    /// `setSelected` with the mouse as the last input: scrolls the row into view when clipped.
    pub fn set_selected(&mut self, i: usize) {
        self.selected = Some(i);
        let (top, h) = (self.row_tops()[i], self.rows[i].h);
        if top + 2 < self.base.y || top + 2 + h - 4 > self.base.y + self.base.h {
            self.scroll_to_row(i);
        }
    }

    /// `scrollToEntry`.
    pub fn scroll_to_row(&mut self, i: usize) {
        let h = self.rows[i].h;
        let above = self.row_tops()[i] - self.base.y - 2;
        if above < 0 {
            self.set_scroll(self.scroll + f64::from(above));
        }
        let below = self.base.y + self.base.h - self.row_tops()[i] - h - 2;
        if below < 0 {
            self.set_scroll(self.scroll - f64::from(below));
        }
    }

    fn scroller_height(&self) -> i32 {
        let h = self.base.h;
        // Mth.clamp: max then min, so a list under 40 high gets h - 8.
        (((h * h) as f32 / self.content_height() as f32) as i32).max(32).min(h - 8)
    }

    pub fn scroll_bar_x(&self) -> i32 {
        self.row_right() + 6 + 2
    }

    /// Every row's widgets placed for the scroll amount, as the next frame draws them.
    pub fn place_rows(&mut self) {
        let (left, width) = (self.row_left(), self.row_width);
        let tops = self.row_tops();
        for (row, top) in self.rows.iter_mut().zip(tops) {
            row.entry.place([left, top, width, row.h]);
        }
    }

    /// The rows' widgets in order, clipped to the list.
    pub fn children(&mut self) -> Vec<Child<'_>> {
        let b = &self.base;
        let clip = Some([b.x, b.y, b.x + b.w, b.y + b.h]);
        self.rows.iter_mut().flat_map(|r| r.entry.children()).map(|widget| Child { widget, clip }).collect()
    }

    /// `isMouseOver`.
    pub fn over(&self, x: f64, y: f64) -> bool {
        let b = &self.base;
        b.is_active() && x >= f64::from(b.x) && y >= f64::from(b.y) && x < f64::from(b.x + b.w) && y < f64::from(b.y + b.h)
    }

    /// `updateScrolling`: a click on the bar of a list that scrolls.
    pub fn over_scrollbar(&self, x: f64, y: f64) -> bool {
        let (bx, b) = (f64::from(self.scroll_bar_x()), &self.base);
        self.max_scroll() > 0 && x >= bx && x <= bx + 6.0 && y >= f64::from(b.y) && y < f64::from(b.y + b.h)
    }

    /// `AbstractScrollArea.mouseDragged` while the bar is held: the mouse at `y`, moved `dy`.
    pub fn drag_scrollbar(&mut self, y: f64, dy: f64) {
        let b = &self.base;
        if y < f64::from(b.y) {
            self.set_scroll(0.0);
        } else if y > f64::from(b.y + b.h) {
            self.set_scroll(f64::from(self.max_scroll()));
        } else {
            let max = f64::from(self.max_scroll().max(1));
            let scale = (max / f64::from(b.h - self.scroller_height())).max(1.0);
            self.set_scroll(self.scroll + dy * scale);
        }
    }

    fn scroll_bar_y(&self) -> i32 {
        let max = self.max_scroll();
        if max == 0 {
            self.base.y
        } else {
            self.base.y.max(self.scroll as i32 * (self.base.h - self.scroller_height()) / max + self.base.y)
        }
    }

    /// `AbstractScrollArea.extractScrollbar` (default settings: no disabled scroller).
    fn scrollbar(&self, ctx: &mut Ctx) {
        if self.max_scroll() > 0 {
            let (x, b) = (self.scroll_bar_x(), &self.base);
            ctx.gui.sprite("widget/scroller_background", x, b.y, 6, b.h);
            ctx.gui.sprite("widget/scroller", x, self.scroll_bar_y(), 6, self.scroller_height());
        }
    }
}

impl Widget for SelectionList {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let b = self.base.clone();
        let (mx, my) = ctx.mouse;
        let tops = self.row_tops();
        let (left, width) = (self.row_left(), self.row_width);
        let over = b.is_active() && b.contains(mx, my);
        let hovered = if over { tops.iter().zip(&self.rows).position(|(&t, r)| mx >= left && mx < left + width && my >= t && my < t + r.h) } else { None };
        let bg = if self.in_world { "gui/inworld_menu_list_background" } else { "gui/menu_list_background" };
        ctx.blit(bg, b.x, b.y, [(b.x + b.w) as f32, (b.y + b.h + self.scroll as i32) as f32], b.w, b.h, [32, 32]);
        let (gw, gh) = (ctx.gui.width, ctx.gui.height);
        ctx.enable_scissor(b.x.clamp(0, gw), b.y.clamp(0, gh), (b.x + b.w).clamp(0, gw), (b.y + b.h).clamp(0, gh));
        for (i, row) in self.rows.iter_mut().enumerate() {
            let top = tops[i];
            if top + row.h >= b.y && top <= b.y + b.h {
                if self.selectable && self.selected == Some(i) {
                    let outline = if self.focused { 0xFFFF_FFFF } else { 0xFF80_8080 };
                    ctx.gui.fill(left, top, left + width, top + row.h, outline);
                    ctx.gui.fill(left + 1, top + 1, left + width - 1, top + row.h - 1, 0xFF00_0000);
                }
                row.entry.extract_content(ctx, [left, top, width, row.h], hovered == Some(i));
            }
        }
        ctx.disable_scissor();
        let (head, foot) = if self.in_world { ("gui/inworld_header_separator", "gui/inworld_footer_separator") } else { ("gui/header_separator", "gui/footer_separator") };
        ctx.blit(head, b.x, b.y - 2, [0.0, 0.0], b.w, 2, [32, 2]);
        ctx.blit(foot, b.x, b.y + b.h, [0.0, 0.0], b.w, 2, [32, 2]);
        self.scrollbar(ctx);
    }
}
