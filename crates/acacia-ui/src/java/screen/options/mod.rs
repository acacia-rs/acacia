//! The options screens: `OptionsScreen` and the `OptionsSubScreen` pages — a header, an
//! [`list::OptionsList`] of option widgets and a footer in a `HeaderAndFooterLayout`.

pub mod defs;
pub mod list;
pub mod pages;
pub mod value;

use super::{Env, Screen, menu_background};
use crate::java::widget::header_footer::HeaderFooter;
use crate::java::widget::layout::{Arena, Elem};
use crate::java::widget::{Ctx, Widget, render};
use list::OptionsList;

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
}
