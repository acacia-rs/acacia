//! Widgets at fixed places, some in a scrolling viewport: mouse and keyboard input, focus, the
//! open dropdown list, and drawing through a [`Skin`].

use std::time::Instant;

use super::{ListGeometry, Skin, State, Widget, contains};
use crate::draw::DrawList;
use crate::font::Font;
use crate::input::{Input, Key, Mods};

/// Caret blink half-period (Java's `TextCursorUtils`).
const BLINK_MS: u128 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub widget: Widget,
    /// GUI pixels; for a scrolling widget, where it lies when the viewport is at the top.
    pub rect: [f32; 4],
    pub scrolls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    Ignored,
    Consumed,
    /// The button at this index was clicked or activated by key.
    Pressed(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Slider(usize),
    /// The scrollbar thumb, grabbed this far below its top.
    Thumb(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Open {
    item: usize,
    scroll: f32,
}

pub struct Panel {
    pub items: Vec<Placed>,
    pub viewport: [f32; 4],
    /// Bottom of the scrolling content, before scrolling.
    content_bottom: f32,
    scroll: f32,
    mouse: [f32; 2],
    focus: Option<usize>,
    focused_at: Instant,
    held: Option<usize>,
    drag: Option<Drag>,
    open: Option<Open>,
}

impl Panel {
    /// `items` placed by the caller; the scrolling ones end `pad` above the content's bottom.
    pub fn new(items: Vec<Placed>, viewport: [f32; 4], pad: f32) -> Panel {
        let content_bottom = items.iter().filter(|p| p.scrolls).map(|p| p.rect[3]).fold(viewport[1], f32::max) + pad;
        Panel { items, viewport, content_bottom, scroll: 0.0, mouse: [-1.0; 2], focus: None, focused_at: Instant::now(), held: None, drag: None, open: None }
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content_bottom - self.viewport[3]).max(0.0)
    }

    /// Where item `i` is now drawn.
    pub fn rect(&self, i: usize) -> [f32; 4] {
        let p = &self.items[i];
        let dy = if p.scrolls { -self.scroll } else { 0.0 };
        [p.rect[0], p.rect[1] + dy, p.rect[2], p.rect[3] + dy]
    }

    fn item_at(&self, skin: &dyn Skin, font: Option<&Font>, at: [f32; 2]) -> Option<usize> {
        (0..self.items.len()).find(|&i| {
            let p = &self.items[i];
            p.widget.is_interactive() && (!p.scrolls || contains(self.viewport, at)) && contains(skin.hit(font, &p.widget, self.rect(i)), at)
        })
    }

    pub fn handle(&mut self, input: &Input, skin: &dyn Skin, font: Option<&Font>) -> Response {
        if let Some(open) = self.open {
            return self.handle_list(open, input, skin, font);
        }
        match input {
            Input::Move(at) => {
                self.mouse = *at;
                match self.drag {
                    Some(Drag::Slider(i)) => self.slide(i, at[0], skin, font),
                    Some(Drag::Thumb(grab)) => self.drag_thumb(at[1] - grab, skin),
                    None => return Response::Ignored,
                }
                Response::Consumed
            }
            Input::Press(at) => self.press(*at, skin, font),
            Input::Release(at) => {
                self.drag = None;
                let held = self.held.take();
                match held {
                    Some(i) if self.item_at(skin, font, *at) == Some(i) && matches!(self.items[i].widget, Widget::Button { .. }) => Response::Pressed(i),
                    Some(_) => Response::Consumed,
                    None => Response::Ignored,
                }
            }
            Input::Wheel(notches) => {
                self.scroll = (self.scroll - notches * skin.scroll_step()).clamp(0.0, self.max_scroll());
                Response::Consumed
            }
            Input::Key(key, mods) => self.key(*key, *mods, skin),
            Input::Text(text) => match self.focus.map(|i| &mut self.items[i].widget) {
                Some(Widget::Input { edit, .. }) => {
                    edit.insert(text);
                    Response::Consumed
                }
                _ => Response::Ignored,
            },
        }
    }

    fn press(&mut self, at: [f32; 2], skin: &dyn Skin, font: Option<&Font>) -> Response {
        self.mouse = at;
        let track = skin.scroll_track(self.viewport);
        if self.max_scroll() > 0.0 && contains(track, at) {
            let thumb = skin.thumb(track, self.viewport[3] - self.viewport[1], self.content_bottom - self.viewport[1], self.scroll / self.max_scroll());
            let grab = if contains(thumb, at) { at[1] - thumb[1] } else { (thumb[3] - thumb[1]) / 2.0 };
            self.drag = Some(Drag::Thumb(grab));
            self.drag_thumb(at[1] - grab, skin);
            return Response::Consumed;
        }
        let Some(i) = self.item_at(skin, font, at) else {
            self.set_focus(None);
            return Response::Ignored;
        };
        self.set_focus(Some(i));
        let cycle = skin.choices_cycle();
        match &mut self.items[i].widget {
            Widget::Button { .. } => self.held = Some(i),
            Widget::Toggle { on, .. } => *on = !*on,
            Widget::Steps { options, index, .. } | Widget::Dropdown { options, index, .. } if cycle => *index = (*index + 1) % options.len().max(1),
            Widget::Dropdown { .. } => self.open = Some(Open { item: i, scroll: 0.0 }),
            Widget::Slider { .. } | Widget::Steps { .. } => {
                self.drag = Some(Drag::Slider(i));
                self.slide(i, at[0], skin, font);
            }
            _ => {}
        }
        Response::Consumed
    }

    fn key(&mut self, key: Key, mods: Mods, skin: &dyn Skin) -> Response {
        let focused = self.focus.map(|i| &mut self.items[i].widget);
        if let Some(Widget::Input { edit, .. }) = focused
            && edit.key(key)
        {
            return Response::Consumed;
        }
        match key {
            Key::Tab => self.move_focus(!mods.shift),
            Key::Down => self.move_focus(true),
            Key::Up => self.move_focus(false),
            Key::Left | Key::Right => {
                let Some(i) = self.focus else { return Response::Ignored };
                self.step(i, if key == Key::Right { 1 } else { -1 });
            }
            Key::Enter | Key::Space => {
                let Some(i) = self.focus else { return Response::Ignored };
                let cycle = skin.choices_cycle();
                match &mut self.items[i].widget {
                    Widget::Button { .. } => return Response::Pressed(i),
                    Widget::Toggle { on, .. } => *on = !*on,
                    Widget::Dropdown { .. } if !cycle => self.open = Some(Open { item: i, scroll: 0.0 }),
                    Widget::Steps { .. } | Widget::Dropdown { .. } if cycle => self.step(i, if mods.shift { -1 } else { 1 }),
                    _ => {}
                }
            }
            _ => return Response::Ignored,
        }
        Response::Consumed
    }

    /// Moves a slider, step slider or dropdown `by` steps.
    fn step(&mut self, i: usize, by: i32) {
        match &mut self.items[i].widget {
            Widget::Slider { min, max, step, value, .. } => *value = (*value + f64::from(by) * *step).clamp(*min, *max),
            Widget::Steps { options, index, .. } | Widget::Dropdown { options, index, .. } => {
                let n = options.len().max(1) as i32;
                *index = (*index as i32 + by).rem_euclid(n) as usize;
            }
            _ => {}
        }
    }

    fn move_focus(&mut self, forward: bool) {
        let order: Vec<usize> = (0..self.items.len()).filter(|&i| self.items[i].widget.is_interactive()).collect();
        if order.is_empty() {
            return;
        }
        let at = self.focus.and_then(|f| order.iter().position(|&i| i == f));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => order.len() - 1,
            (Some(p), true) => (p + 1) % order.len(),
            (Some(p), false) => (p + order.len() - 1) % order.len(),
        };
        self.set_focus(Some(order[next]));
        self.reveal(order[next]);
    }

    fn set_focus(&mut self, focus: Option<usize>) {
        if self.focus != focus {
            self.focus = focus;
            self.focused_at = Instant::now();
        }
    }

    /// Scrolls just enough to show item `i` whole.
    fn reveal(&mut self, i: usize) {
        let p = &self.items[i];
        if !p.scrolls {
            return;
        }
        let [_, top, _, bottom] = p.rect;
        let (vt, vb) = (self.viewport[1], self.viewport[3]);
        if top - self.scroll < vt {
            self.scroll = top - vt;
        } else if bottom - self.scroll > vb {
            self.scroll = bottom - vb;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn slide(&mut self, i: usize, x: f32, skin: &dyn Skin, font: Option<&Font>) {
        let [a, b] = skin.slider_span(font, self.rect(i));
        let t = f64::from(((x - a) / (b - a).max(1.0)).clamp(0.0, 1.0));
        match &mut self.items[i].widget {
            Widget::Slider { min, max, step, value, .. } => {
                let raw = *min + t * (*max - *min);
                let snapped = if *step > 0.0 { *min + ((raw - *min) / *step).round() * *step } else { raw };
                *value = snapped.clamp(*min, *max);
            }
            Widget::Steps { options, index, .. } => *index = (t * (options.len().max(1) - 1) as f64).round() as usize,
            _ => {}
        }
    }

    fn drag_thumb(&mut self, thumb_top: f32, skin: &dyn Skin) {
        let track = skin.scroll_track(self.viewport);
        let full = skin.thumb(track, self.viewport[3] - self.viewport[1], self.content_bottom - self.viewport[1], 0.0);
        let travel = (track[3] - track[1]) - (full[3] - full[1]);
        if travel > 0.0 {
            self.scroll = ((thumb_top - track[1]) / travel).clamp(0.0, 1.0) * self.max_scroll();
        }
    }

    /// Input while a dropdown's list is open: it takes everything.
    fn handle_list(&mut self, open: Open, input: &Input, skin: &dyn Skin, font: Option<&Font>) -> Response {
        let Widget::Dropdown { options, .. } = &self.items[open.item].widget else {
            self.open = None;
            return Response::Consumed;
        };
        let count = options.len();
        let g = skin.dropdown_list(font, self.rect(open.item), count);
        match input {
            Input::Move(at) => self.mouse = *at,
            Input::Press(at) => {
                if let Some(row) = list_row(g, open.scroll, count, *at)
                    && let Widget::Dropdown { index, .. } = &mut self.items[open.item].widget
                {
                    *index = row;
                }
                self.open = None;
            }
            Input::Wheel(n) => {
                let content = g.first[1] - g.rect[1] + count as f32 * g.row + 2.0;
                let max = (content - (g.rect[3] - g.rect[1])).max(0.0);
                self.open = Some(Open { scroll: (open.scroll - n * g.row).clamp(0.0, max), ..open });
            }
            Input::Key(Key::Up, _) => self.step(open.item, -1),
            Input::Key(Key::Down, _) => self.step(open.item, 1),
            Input::Key(Key::Escape | Key::Enter | Key::Space | Key::Tab, _) => self.open = None,
            _ => {}
        }
        Response::Consumed
    }

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
            let track = skin.scroll_track(self.viewport);
            let thumb = skin.thumb(track, self.viewport[3] - self.viewport[1], self.content_bottom - self.viewport[1], self.scroll / self.max_scroll());
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

    /// The values of the input widgets, in order.
    pub fn values(&self) -> Vec<super::Value> {
        self.items.iter().filter_map(|p| p.widget.value()).collect()
    }
}

/// The open list's row under `at`.
fn list_row(g: ListGeometry, scroll: f32, count: usize, at: [f32; 2]) -> Option<usize> {
    let row = ((at[1] - g.first[1] + scroll) / g.row).floor();
    (contains(g.rect, at) && row >= 0.0 && (row as usize) < count).then_some(row as usize)
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
