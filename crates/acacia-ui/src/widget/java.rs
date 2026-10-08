//! Java's widgets from the jar's `textures/gui/sprites/widget`: 20 px buttons, checkboxes, sliders,
//! edit boxes, and cycle buttons for every choice. Numbers: research/forms-java-widgets.md §2.
//! A missing sprite is drawn as a flat fill of its main colour.

use std::path::Path;

use super::bedrock::rgba;
use super::{ListGeometry, Skin, State, Widget, clipped, draw_lines, lines};
use crate::atlas::{Atlas, Sprite};
use crate::draw::{DrawList, WHITE as OPAQUE};
use crate::font::{Font, LINE_HEIGHT};
use crate::nine::Nine;

const WHITE: u32 = 0xFFFFFF;
const FIELD_TEXT: u32 = 0xE0E0E0;
/// `ChatFormatting.DARK_GRAY`, the edit box hint [unverified].
const HINT: u32 = 0x555555;
pub const HEIGHT: f32 = 20.0;
const CHECKBOX: f32 = 17.0;
const HANDLE: f32 = 8.0;
/// Label above an edit box (`CommonLayouts.labeledElement`).
const LABEL_GAP: f32 = 4.0;
/// Padding of a `FocusableTextWidget`.
const TEXT_PAD: f32 = 4.0;
const SCROLLBAR: f32 = 6.0;

pub struct Kit {
    white: Sprite,
    /// Normal, highlighted.
    button: [Option<Nine>; 2],
    /// Plain, highlighted, selected, selected and highlighted.
    checkbox: [Option<Nine>; 4],
    slider: [Option<Nine>; 2],
    handle: [Option<Nine>; 2],
    field: [Option<Nine>; 2],
    scroller: [Option<Nine>; 2],
    /// `container/inventory.png`'s 176×166 screen; `generic_54.png`'s six rows and title (176×125)
    /// and its player part (176×96, from y 126).
    pub inventory: Option<Sprite>,
    pub chest_top: Option<Sprite>,
    pub chest_bottom: Option<Sprite>,
}

impl Kit {
    pub fn load(root: &Path, atlas: &mut Atlas) -> Kit {
        let container = |file: &str, [x, y, w, h]: [u32; 4], atlas: &mut Atlas| {
            let image = crate::theme::png(&root.join(format!("textures/gui/container/{file}.png")))?;
            // The sheets are 256×256 at any pack resolution; crop in its units.
            let s = image.width() / 256;
            Some(atlas.add(&format!("container/{file}/{y}"), &image::imageops::crop_imm(&image, x * s, y * s, w * s, h * s).to_image()))
        };
        let inventory = container("inventory", [0, 0, 176, 166], atlas);
        let chest_top = container("generic_54", [0, 0, 176, 125], atlas);
        let chest_bottom = container("generic_54", [0, 126, 176, 96], atlas);
        let mut n = |name: &str| Nine::java(root, &format!("widget/{name}"), atlas);
        Kit {
            inventory,
            chest_top,
            chest_bottom,
            button: [n("button"), n("button_highlighted")],
            checkbox: [n("checkbox"), n("checkbox_highlighted"), n("checkbox_selected"), n("checkbox_selected_highlighted")],
            slider: [n("slider"), n("slider_highlighted")],
            handle: [n("slider_handle"), n("slider_handle_highlighted")],
            field: [n("text_field"), n("text_field_highlighted")],
            scroller: [n("scroller_background"), n("scroller")],
            white: atlas.white(),
        }
    }

    fn paint(&self, list: &mut DrawList, nine: Option<Nine>, rect: [f32; 4], fallback: u32) {
        match nine {
            Some(nine) => list.nine(nine, rect, OPAQUE),
            None => list.fill(self.white, rect, rgba(fallback, 255)),
        }
    }

    /// Height of `widget` in a column `width` wide.
    pub fn height(&self, font: Option<&Font>, widget: &Widget, width: f32) -> f32 {
        match widget {
            Widget::Text { text } | Widget::Header { text } => lines(font, text, width - 2.0 * TEXT_PAD).len() as f32 * LINE_HEIGHT + 2.0 * TEXT_PAD,
            Widget::Divider => 2.0,
            Widget::Toggle { label, .. } => (lines(font, label, width - CHECKBOX - 4.0).len().min(2) as f32 * LINE_HEIGHT).max(CHECKBOX),
            Widget::Input { .. } => LINE_HEIGHT + LABEL_GAP + HEIGHT,
            _ => HEIGHT,
        }
    }

    fn field_rect(rect: [f32; 4]) -> [f32; 4] {
        [rect[0], rect[3] - HEIGHT, rect[2], rect[3]]
    }

    /// A button face with its label centred on one line (Java scrolls a long one; it is clipped here).
    fn button(&self, list: &mut DrawList, font: Option<&Font>, text: &str, image: Option<Sprite>, rect: [f32; 4], lit: bool) {
        self.paint(list, self.button[usize::from(lit)], rect, if lit { 0x757575 } else { 0x6F6F6F });
        let mut area = [rect[0] + 2.0, rect[1], rect[2] - 2.0, rect[3]];
        if let Some(sprite) = image {
            let y = ((rect[1] + rect[3]) / 2.0 - 8.0).floor();
            list.sprite_stretched(sprite, [rect[0] + 2.0, y, rect[0] + 18.0, y + 16.0], OPAQUE);
            area[0] += 18.0;
        }
        let Some(font) = font else { return };
        let y = ((rect[1] + rect[3] - LINE_HEIGHT) / 2.0).floor() + 1.0;
        let x = ((area[0] + area[2]) / 2.0 - font.width(text) / 2.0).floor().max(area[0]);
        clipped(list, area, |list| {
            font.draw(list, text, x, y, WHITE, 1.0, true);
        });
    }
}

impl Skin for Kit {
    fn draw(&self, list: &mut DrawList, font: Option<&Font>, widget: &Widget, rect: [f32; 4], state: State) {
        let [l, t, r, b] = rect;
        let lit = state.hover || state.focus;
        match widget {
            Widget::Text { text } => {
                let wrapped = lines(font, text, r - l - 2.0 * TEXT_PAD);
                draw_lines(list, font, &wrapped, (l + r) / 2.0, t + TEXT_PAD, LINE_HEIGHT, true, WHITE, true);
            }
            Widget::Header { text } => {
                let wrapped = lines(font, &format!("§l{text}"), r - l - 2.0 * TEXT_PAD);
                draw_lines(list, font, &wrapped, (l + r) / 2.0, t + TEXT_PAD, LINE_HEIGHT, true, WHITE, true);
            }
            Widget::Divider => {
                list.fill(self.white, [l, t, r, t + 1.0], [255, 255, 255, 0x33]);
                list.fill(self.white, [l, t + 1.0, r, t + 2.0], [0, 0, 0, 0xBF]);
            }
            Widget::Button { text, image } => self.button(list, font, text, *image, rect, lit),
            Widget::Steps { .. } | Widget::Dropdown { .. } => {
                self.button(list, font, &widget.label_with_value().unwrap_or_default(), None, rect, lit);
            }
            Widget::Toggle { label, on } => {
                let box_rect = [l, t + ((b - t - CHECKBOX) / 2.0).floor(), l + CHECKBOX, t + ((b - t - CHECKBOX) / 2.0).floor() + CHECKBOX];
                self.paint(list, self.checkbox[usize::from(*on) * 2 + usize::from(state.focus)], box_rect, 0x2C2C2C);
                let mut wrapped = lines(font, label, r - l - CHECKBOX - 4.0);
                wrapped.truncate(2);
                let y = ((t + b) / 2.0 - wrapped.len() as f32 * LINE_HEIGHT / 2.0).floor();
                draw_lines(list, font, &wrapped, l + CHECKBOX + 4.0, y, LINE_HEIGHT, false, WHITE, true);
            }
            Widget::Slider { min, max, value, .. } => {
                self.paint(list, self.slider[usize::from(state.focus && !state.hover)], rect, 0x2C2C2C);
                let at = if max > min { ((value - min) / (max - min)) as f32 } else { 0.0 };
                let x = l + (at.clamp(0.0, 1.0) * (r - l - HANDLE)).floor();
                self.paint(list, self.handle[usize::from(state.hover || state.pressed)], [x, t, x + HANDLE, b], 0x6F6F6F);
                let text = widget.label_with_value().unwrap_or_default();
                if let Some(font) = font {
                    let tx = ((l + r) / 2.0 - font.width(&text) / 2.0).floor();
                    clipped(list, [l + 2.0, t, r - 2.0, b], |list| {
                        font.draw(list, &text, tx, ((t + b - LINE_HEIGHT) / 2.0).floor() + 1.0, WHITE, 1.0, true);
                    });
                }
            }
            Widget::Input { label, placeholder, edit } => {
                draw_lines(list, font, &[label.clone()], l, t, LINE_HEIGHT, false, WHITE, true);
                let field = Kit::field_rect(rect);
                self.paint(list, self.field[usize::from(state.focus)], field, 0x000000);
                let area = [field[0] + 4.0, field[1], field[2] - 4.0, field[3]];
                let y = field[1] + ((HEIGHT - 8.0) / 2.0).floor();
                let Some(font) = font else { return };
                clipped(list, area, |list| {
                    if edit.is_empty() && !state.focus {
                        font.draw(list, placeholder, area[0], y, HINT, 1.0, true);
                        return;
                    }
                    let caret = font.width(&edit.before_caret());
                    let x = area[0] - (caret + 1.0 - (area[2] - area[0])).max(0.0);
                    font.draw(list, &edit.text(), x, y, FIELD_TEXT, 1.0, true);
                    if state.caret {
                        list.fill(self.white, [x + caret, y - 1.0, x + caret + 1.0, y + 9.0], rgba(0xD0D0D0, 255));
                    }
                });
            }
        }
    }

    fn hit(&self, font: Option<&Font>, widget: &Widget, rect: [f32; 4]) -> [f32; 4] {
        match widget {
            Widget::Input { .. } => Kit::field_rect(rect),
            Widget::Toggle { label, .. } => {
                let text = font.map_or(0.0, |f| lines(Some(f), label, rect[2] - rect[0] - CHECKBOX - 4.0).iter().map(|l| f.width(l)).fold(0.0, f32::max));
                [rect[0], rect[1], (rect[0] + CHECKBOX + 4.0 + text).min(rect[2]), rect[3]]
            }
            _ => rect,
        }
    }

    fn slider_span(&self, _font: Option<&Font>, rect: [f32; 4]) -> [f32; 2] {
        [rect[0] + HANDLE / 2.0, rect[2] - HANDLE / 2.0]
    }

    fn choices_cycle(&self) -> bool {
        true
    }

    fn dropdown_list(&self, _font: Option<&Font>, rect: [f32; 4], _options: usize) -> ListGeometry {
        ListGeometry { rect: [0.0; 4], first: [rect[0], rect[1]], row: HEIGHT, row_width: 0.0 }
    }

    fn draw_dropdown_list(&self, _: &mut DrawList, _: Option<&Font>, _: ListGeometry, _: &[String], _: usize, _: Option<usize>, _: f32) {}

    fn scroll_track(&self, viewport: [f32; 4]) -> [f32; 4] {
        [viewport[2] - SCROLLBAR, viewport[1], viewport[2], viewport[3]]
    }

    fn thumb(&self, track: [f32; 4], visible: f32, content: f32, scroll: f32) -> [f32; 4] {
        let h = track[3] - track[1];
        // `Mth.clamp(v, 32, h - 8)`: never panics on a short viewport, unlike `f32::clamp`.
        let v = (h * visible / content.max(visible)).floor();
        let thumb = if v < 32.0 { 32.0 } else { v.min(h - 8.0) }.min(h).max(1.0);
        let top = track[1] + (scroll * (h - thumb)).floor();
        [track[0], top, track[2], top + thumb]
    }

    fn draw_scrollbar(&self, list: &mut DrawList, track: [f32; 4], thumb: [f32; 4]) {
        self.paint(list, self.scroller[0], track, 0x000000);
        self.paint(list, self.scroller[1], thumb, 0xC0C0C0);
    }

    fn scroll_step(&self) -> f32 {
        10.0
    }
}
