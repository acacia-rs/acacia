//! The options screens: `OptionsScreen` and the `OptionsSubScreen` pages — a header, an
//! [`list::OptionsList`] of option widgets and a footer in a `HeaderAndFooterLayout`.

pub mod defs;
pub mod input;
pub mod list;
pub mod pages;
pub mod value;

use super::{Env, Screen, arena_children, menu_background};
use crate::java::widget::header_footer::HeaderFooter;
use crate::java::widget::input::{self as widget_input, Child, Fired, Interaction, Key};
use crate::java::widget::layout::{Arena, Elem};
use crate::java::widget::{Ctx, Widget, render};
use list::{OptionsList, SCROLL_RATE};

/// An options screen after `init`: its widgets in the layout's order, the list among them.
pub struct OptionsScreen {
    pub env: Env,
    widgets: Arena,
    /// Widgets before the list's place and after it.
    split: usize,
    pub list: Option<OptionsList>,
}

/// What a screen's `init` builds: the layout over the widgets it holds, and the list if any.
pub struct Build {
    pub widgets: Arena,
    pub layout: HeaderFooter,
    pub list: Option<OptionsList>,
}

impl Build {
    /// `new HeaderAndFooterLayout(screen, header, footer)`.
    pub fn new(header_h: i32, footer_h: i32) -> Build {
        Build { widgets: Vec::new(), layout: HeaderFooter::new(header_h, footer_h), list: None }
    }

    /// The widget pushed, as a layout element.
    pub fn add(&mut self, w: impl Widget + 'static) -> Elem {
        self.widgets.push(Box::new(w));
        Elem::Widget(self.widgets.len() - 1)
    }

    /// `OptionsSubScreen.addContents`: the list fills the contents.
    pub fn add_list(&mut self, env: &Env) -> &mut OptionsList {
        let h = self.layout.content_height(env.height);
        self.layout.add_contents(Elem::Spacer { x: 0, y: 0, w: env.width, h });
        self.list.insert(OptionsList::new(env.width, h, self.layout.header_h, env.in_world))
    }

    /// `layout.visitWidgets(addRenderableWidget)` and `repositionElements`.
    pub fn finish(mut self, env: Env) -> OptionsScreen {
        self.layout.arrange(&mut self.widgets, env.width, env.height);
        if let Some(list) = &mut self.list {
            list.update_size(env.width, self.layout.content_height(env.height), self.layout.header_h);
        }
        let mut order = Vec::new();
        self.layout.header.visit_widgets(&mut order);
        self.layout.contents.visit_widgets(&mut order);
        let split = order.len();
        self.layout.footer.visit_widgets(&mut order);
        let mut slots: Vec<Option<Box<dyn Widget>>> = self.widgets.into_iter().map(Some).collect();
        let widgets = order.into_iter().filter_map(|i| slots[i].take()).collect();
        OptionsScreen { env, widgets, split, list: self.list }
    }
}

impl Screen for OptionsScreen {
    fn background(&mut self, ctx: &mut Ctx) {
        menu_background(&self.env, ctx);
    }

    fn foreground(&mut self, ctx: &mut Ctx) {
        let (before, after) = self.widgets.split_at_mut(self.split);
        for w in before {
            render(w.as_mut(), ctx);
        }
        if let Some(list) = &mut self.list {
            render(&mut list.list, ctx);
        }
        for w in after {
            render(w.as_mut(), ctx);
        }
    }

    /// The header's widgets, the list's rows, the footer's.
    fn children(&mut self) -> Vec<Child<'_>> {
        let (before, after) = self.widgets.split_at_mut(self.split);
        let mut out = arena_children(before);
        out.extend(self.list.iter_mut().flat_map(|l| l.list.children()));
        out.extend(arena_children(after));
        out
    }

    fn restore(&mut self, s: &Interaction) {
        if let Some(l) = &mut self.list {
            l.list.set_scroll(s.scroll);
            l.list.place_rows();
        }
        widget_input::restore(&mut self.children(), s);
    }

    /// A click on the list's bar holds it (`updateScrolling`) and takes the focus off the widgets.
    fn mouse_clicked(&mut self, s: &mut Interaction, x: f64, y: f64, shift: bool) -> Option<Fired> {
        if self.list.as_ref().is_some_and(|l| l.list.over(x, y) && l.list.over_scrollbar(x, y)) {
            widget_input::focus(&mut self.children(), s, None, true);
            (s.scrolling, s.dragging) = (true, true);
            return None;
        }
        widget_input::mouse_clicked(&mut self.children(), s, x, y, shift)
    }

    fn mouse_released(&mut self, s: &mut Interaction) {
        s.scrolling = false;
        widget_input::mouse_released(&mut self.children(), s);
    }

    fn mouse_dragged(&mut self, s: &mut Interaction, x: f64, y: f64, dy: f64) -> Option<Fired> {
        if s.scrolling {
            if let Some(l) = &mut self.list {
                l.list.drag_scrollbar(y, dy);
                s.scroll = l.list.scroll;
            }
            return None;
        }
        widget_input::mouse_dragged(&mut self.children(), s, x)
    }

    fn mouse_scrolled(&mut self, s: &mut Interaction, x: f64, y: f64, amount: f64) -> bool {
        let Some(l) = self.list.as_mut().filter(|l| l.list.over(x, y)) else { return false };
        l.list.set_scroll(l.list.scroll - amount * SCROLL_RATE);
        s.scroll = l.list.scroll;
        true
    }

    /// A row focused from the keyboard scrolls into view (`setSelected` after keyboard input).
    fn key_pressed(&mut self, s: &mut Interaction, key: Key, shift: bool) -> Option<Fired> {
        let before = s.focused;
        let fired = widget_input::key_pressed(&mut self.children(), s, key, shift);
        let split = self.split;
        if let (Some(i), Some(l)) = (s.focused.filter(|&i| i >= split && s.focused != before), &mut self.list)
            && let Some(row) = l.row_of(i - split)
        {
            l.list.scroll_to_row(row);
            s.scroll = l.list.scroll;
        }
        fired
    }
}
