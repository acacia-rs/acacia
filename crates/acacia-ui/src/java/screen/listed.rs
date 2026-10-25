//! A screen of a `HeaderAndFooterLayout` around selection lists (key binds, languages, packs): the
//! header's widgets, the lists, the footer's widgets, drawn and taking input in that order.

use super::{Env, Screen, arena_children, menu_background};
use crate::java::widget::header_footer::HeaderFooter;
use crate::java::widget::input::{self as widget_input, Child, Fired, Interaction, Key};
use crate::java::widget::layout::{Arena, Elem};
use crate::java::widget::list::SelectionList;
use crate::java::widget::{Ctx, Widget, render};

pub struct ListedScreen {
    pub env: Env,
    pub widgets: Arena,
    /// Widgets before the lists' place and after it.
    pub split: usize,
    pub lists: Vec<SelectionList>,
    /// Each list's `defaultEntryHeight`.
    row_heights: Vec<i32>,
}

/// What a screen's `init` builds before `finish`.
pub struct Build {
    pub widgets: Arena,
    pub layout: HeaderFooter,
}

impl Build {
    pub fn new(header_h: i32, footer_h: i32) -> Build {
        Build { widgets: Vec::new(), layout: HeaderFooter::new(header_h, footer_h) }
    }

    /// The widget pushed, as a layout element.
    pub fn add(&mut self, w: impl Widget + 'static) -> Elem {
        self.widgets.push(Box::new(w));
        Elem::Widget(self.widgets.len() - 1)
    }

    /// `layout.arrangeElements` and `visitWidgets(addRenderableWidget)`: the widgets in the
    /// header's, contents' and footer's order (the lists sit in the contents as spacers).
    /// `lists` with their `defaultEntryHeight`.
    pub fn finish(mut self, env: Env, lists: Vec<(SelectionList, i32)>) -> ListedScreen {
        let (lists, row_heights) = lists.into_iter().unzip();
        self.layout.arrange(&mut self.widgets, env.width, env.height);
        let mut order = Vec::new();
        self.layout.header.visit_widgets(&mut order);
        self.layout.contents.visit_widgets(&mut order);
        let split = order.len();
        self.layout.footer.visit_widgets(&mut order);
        let mut slots: Vec<Option<Box<dyn Widget>>> = self.widgets.into_iter().map(Some).collect();
        let widgets = order.into_iter().filter_map(|i| slots[i].take()).collect();
        ListedScreen { env, widgets, split, lists, row_heights }
    }
}

impl ListedScreen {
    /// The widget known by `id` (the screen's own, not the lists' rows).
    pub fn widget(&mut self, id: &str) -> Option<&mut Box<dyn Widget>> {
        self.widgets.iter_mut().find(|w| w.base().id == id)
    }

    fn list_at(&self, x: f64, y: f64) -> Option<usize> {
        self.lists.iter().position(|l| l.over(x, y))
    }
}

impl Screen for ListedScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        menu_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        let (before, after) = self.widgets.split_at_mut(self.split);
        for w in before {
            render(w.as_mut(), ctx);
        }
        for l in &mut self.lists {
            render(l, ctx);
        }
        for w in after {
            render(w.as_mut(), ctx);
        }
    }

    fn children(&mut self) -> Vec<Child<'_>> {
        let (before, after) = self.widgets.split_at_mut(self.split);
        let mut out = arena_children(before);
        out.extend(self.lists.iter_mut().flat_map(|l| l.children()));
        out.extend(arena_children(after));
        out
    }

    /// The first list's scroll. TODO: a second list's scroll (packs) is not kept across rebuilds.
    fn restore(&mut self, s: &Interaction) {
        if let Some(l) = self.lists.first_mut() {
            l.set_scroll(s.scroll);
            l.place_rows();
        }
        widget_input::restore(&mut self.children(), s);
    }

    /// A list's bar holds it; a click on an entry without widgets selects it and fires its own.
    fn mouse_clicked(&mut self, s: &mut Interaction, x: f64, y: f64, shift: bool) -> Option<Fired> {
        if let Some(i) = self.list_at(x, y) {
            let l = &mut self.lists[i];
            if l.over_scrollbar(x, y) {
                (s.scrolling, s.dragging) = (true, true);
                return None;
            }
            if let Some(row) = l.row_at(x as i32, y as i32)
                && l.rows[row].entry.children().is_empty()
            {
                if l.selectable {
                    l.selected = Some(row);
                    l.focused = true;
                }
                return l.rows[row].entry.clicked(x, y);
            }
        }
        widget_input::mouse_clicked(&mut self.children(), s, x, y, shift)
    }

    fn mouse_released(&mut self, s: &mut Interaction) {
        s.scrolling = false;
        widget_input::mouse_released(&mut self.children(), s);
    }

    fn mouse_dragged(&mut self, s: &mut Interaction, x: f64, y: f64, dy: f64) -> Option<Fired> {
        if s.scrolling {
            if let Some(l) = self.lists.first_mut() {
                l.drag_scrollbar(y, dy);
                s.scroll = l.scroll;
            }
            return None;
        }
        widget_input::mouse_dragged(&mut self.children(), s, x)
    }

    /// `AbstractScrollArea.mouseScrolled`: `scrollRate` is half the default row height.
    fn mouse_scrolled(&mut self, s: &mut Interaction, x: f64, y: f64, amount: f64) -> bool {
        let Some(i) = self.list_at(x, y) else { return false };
        let l = &mut self.lists[i];
        let rate = f64::from(self.row_heights[i] / 2);
        l.set_scroll(l.scroll - amount * rate);
        if i == 0 {
            s.scroll = l.scroll;
        }
        true
    }

    fn key_pressed(&mut self, s: &mut Interaction, key: Key, shift: bool) -> Option<Fired> {
        widget_input::key_pressed(&mut self.children(), s, key, shift)
    }
}
