//! Drawing the panel: its widgets clipped to the viewport, the scrollbar, the open list on top.

use std::time::Instant;

use super::{BLINK_MS, Drag, Panel, list_row};
use crate::draw::DrawList;
use crate::font::Font;
use crate::widget::{Skin, State, Widget};

impl Panel {
    pub fn draw(&self, list: &mut DrawList, skin: &dyn Skin, font: Option<&Font>, now: Instant) {
        let blink = (now.saturating_duration_since(self.focused_at).as_millis() / BLINK_MS).is_multiple_of(2);
        let hover = if self.open.is_some() || self.drag.is_some() { None } else { self.item_at(skin, font, self.mouse) };
        for (i, p) in self.items.iter().enumerate() {
            let rect = self.rect(i);
            if p.scrolls && (rect[3] <= self.viewport[1] || rect[1] >= self.viewport[3]) {
                continue;
            }
            list.clip = p.scrolls.then_some(self.viewport);
            let focus = self.focus == Some(i);
            let state = State {
                hover: hover == Some(i) || self.drag == Some(Drag::Slider(i)),
                focus,
                pressed: self.held == Some(i) && hover == Some(i),
                open: self.open.is_some_and(|o| o.item == i),
                caret: focus && blink,
            };
            skin.draw(list, font, &p.widget, rect, state);
        }
        list.clip = None;
        if self.max_scroll() > 0.0 {
            let (track, thumb) = self.scrollbar(skin);
            skin.draw_scrollbar(list, track, thumb);
        }
        if let Some(open) = self.open
            && let Widget::Dropdown { options, index, .. } = &self.items[open.item].widget
        {
            let g = skin.dropdown_list(font, self.rect(open.item), options.len());
            let hover = list_row(g, open.scroll, options.len(), self.mouse);
            skin.draw_dropdown_list(list, font, g, options, *index, hover, open.scroll);
        }
    }
}
