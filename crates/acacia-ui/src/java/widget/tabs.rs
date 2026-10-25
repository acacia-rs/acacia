//! `MenuTabBar` and its `MenuTabButton`s: the tab row across the top of a tabbed screen.

use super::button::Sprites;
use super::{Base, Ctx, Widget};
use crate::java::text::Text;

const HEIGHT: i32 = 24;

/// `MenuTabBar.MenuTabButton`.
#[derive(Clone, Debug)]
pub struct TabButton {
    pub base: Base,
    pub selected: bool,
}

impl Widget for TabButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let b = self.base.clone();
        let sprites = Sprites("widget/tab_selected".into(), "widget/tab".into(), "widget/tab_selected_highlighted".into(), "widget/tab_highlighted".into());
        ctx.gui.sprite(sprites.get(self.selected, hovered || b.focused), b.x, b.y, b.w, b.h);
        let message = b.shown_message();
        if self.selected {
            ctx.blit("gui/menu_background", b.x + 2, b.y + 2, [0.0, 0.0], b.w - 4, b.h - 2, [32, 32]);
            let color = if b.active { 0xFFFF_FFFF } else { super::INACTIVE };
            let w = ctx.width(&message).min(b.w - 4);
            let (left, top) = (b.x + (b.w - w) / 2, b.y + b.h - 2);
            ctx.gui.fill(left, top, left + w, top + 1, color);
        }
        let (left, right) = (b.x + 1, b.x + b.w - 1);
        let top = b.y + if self.selected { 0 } else { 3 };
        ctx.gui.scrolling(message, (left + right) / 2, left, right, top, b.y + b.h, crate::java::mth::white(b.alpha));
    }
}

/// `MenuTabBar` after `arrangeElements(width)`.
pub struct MenuTabBar {
    pub base: Base,
    pub tabs: Vec<TabButton>,
}

impl MenuTabBar {
    pub fn new(titles: Vec<Text>, selected: usize, width: i32) -> MenuTabBar {
        let tabs_width = width.min(400) - 28;
        let tab_width = round_toward(tabs_width / titles.len() as i32, 2);
        let x0 = round_toward((width - tabs_width) / 2, 2);
        let tabs = titles.into_iter().enumerate().map(|(i, t)| {
            TabButton { base: Base::new(x0 + i as i32 * tab_width, 0, tab_width, HEIGHT, t), selected: i == selected }
        }).collect();
        MenuTabBar { base: Base::new(0, 0, width, HEIGHT, Text::default()), tabs }
    }
}

/// `Mth.roundToward`: up to a multiple of `factor`.
pub fn round_toward(v: i32, factor: i32) -> i32 {
    (v + factor - 1).div_euclid(factor) * factor
}

impl Widget for MenuTabBar {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let y = HEIGHT - 2;
        let first = self.tabs.first().map_or(0, |t| t.base.x);
        ctx.blit("gui/header_separator", 0, y, [0.0, 0.0], first, 2, [32, 2]);
        let after = self.tabs.last().map_or(0, |t| t.base.x + t.base.w);
        ctx.blit("gui/header_separator", after, y, [0.0, 0.0], self.base.w, 2, [32, 2]);
        for tab in &mut self.tabs {
            super::render(tab, ctx);
        }
    }
}
