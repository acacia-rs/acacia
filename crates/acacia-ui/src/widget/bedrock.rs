//! Bedrock's controls from the pack's `textures/ui` (the light buttons, toggles, sliders, edit boxes
//! and dropdowns of `settings_common.json`). Numbers: research/forms-bedrock-layout.md §2.4-2.5, §3.
//! A missing texture is drawn as a flat fill of its main colour.

mod draw;

use std::path::Path;

use super::{ListGeometry, Skin, State, Widget, lines};
use crate::atlas::{Atlas, Sprite};
use crate::draw::DrawList;
use crate::font::Font;
use crate::nine::Nine;

/// GUI pixels between lines of form text [inferred: single-line boxes are 10 tall].
pub const LINE: f32 = 10.0;
/// `$title_text_color` and the light button's text, 0.3 grey.
pub const GREY: u32 = 0x4D4D4D;
const WHITE: u32 = 0xFFFFFF;
/// An open dropdown's text, 0.1216 grey.
const DARK: u32 = 0x1F1F1F;
const PLACEHOLDER: u32 = 0xD9D9D9;
/// `$light_border_default_color`.
const BORDER: [u8; 4] = [0x13, 0x13, 0x13, 0xFF];
/// Gap after each toggle, slider, dropdown and input row.
pub const SPACER: f32 = 4.0;
/// Label to control in slider, dropdown and input rows.
const LABEL_GAP: f32 = 2.0;
const BOX_HEIGHT: f32 = 30.0;
const SLIDER_HEIGHT: f32 = 16.0;
const TOGGLE: [f32; 2] = [30.0, 16.0];
const LIST_HEIGHT: f32 = 60.0;
const LIST_ROW: f32 = 17.0;

pub struct Kit {
    white: Sprite,
    button: [Option<Nine>; 4],
    border: Option<Nine>,
    toggle: [Option<Nine>; 4],
    slider_border: Option<Nine>,
    slider_back: [Option<Nine>; 2],
    slider_fill: [Option<Nine>; 2],
    handle: [Option<Nine>; 3],
    edit: [Option<Nine>; 2],
    list_back: Option<Nine>,
    chevron: [Option<Nine>; 2],
    radio: [Option<Nine>; 4],
    rail: Option<Nine>,
    thumb: Option<Nine>,
    /// The form dialog's frame and hole fill, and the close button's three states.
    pub dialog: Option<Nine>,
    pub hole: Option<Nine>,
    pub close: [Option<Nine>; 3],
    /// Container screens: the panel, a slot, the hovered slot (at 0.8), and the empty armour and
    /// offhand slots' outlines (helmet, chestplate, leggings, boots, shield).
    pub panel: Option<Nine>,
    pub cell: Option<Nine>,
    pub highlight: Option<Nine>,
    pub empty_slots: [Option<Nine>; 5],
    /// A furnace's flame and arrow and a brewing stand's arrow and fuel bar, each empty then full.
    pub flame: [Option<Nine>; 2],
    pub arrow: [Option<Nine>; 2],
    pub brew_arrow: [Option<Nine>; 2],
    pub brew_fuel: [Option<Nine>; 2],
}

impl Kit {
    pub fn load(root: &Path, atlas: &mut Atlas) -> Kit {
        let mut n = |name: &str| Nine::bedrock(root, name, atlas);
        Kit {
            button: [n("button_borderless_light"), n("button_borderless_lighthover"), n("button_borderless_lightpressed"), n("button_borderless_lightpressednohover")],
            border: n("focus_border_white"),
            toggle: [n("toggle_off"), n("toggle_off_hover"), n("toggle_on"), n("toggle_on_hover")],
            slider_border: n("slider_border"),
            slider_back: [n("slider_background"), n("slider_background_hover")],
            slider_fill: [n("slider_progress"), n("slider_progress_hover")],
            handle: [n("slider_button_default"), n("slider_button_hover"), n("slider_button_indent")],
            edit: [n("edit_box_indent"), n("edit_box_indent_hover")],
            list_back: n("dropdown_background"),
            chevron: [n("dropdown_chevron"), n("chevron_white_down")],
            radio: [n("radio_off"), n("radio_off_hover"), n("radio_on"), n("radio_on_hover")],
            rail: n("ScrollRail"),
            thumb: n("ScrollHandle"),
            dialog: n("dialog_background_hollow_3"),
            hole: n("control"),
            close: [n("close_button_default"), n("close_button_hover"), n("close_button_pressed")],
            panel: n("dialog_background_opaque"),
            cell: n("cell_image"),
            highlight: n("highlight_slot"),
            flame: [n("flame_empty_image"), n("flame_full_image")],
            arrow: [n("arrow_inactive"), n("arrow_active")],
            brew_arrow: [n("brewing_arrow_empty"), n("brewing_arrow_full")],
            brew_fuel: [n("brewing_fuel_bar_empty"), n("brewing_fuel_bar_full")],
            empty_slots: ["helmet", "chestplate", "leggings", "boots", "shield"].map(|s| n(&format!("empty_armor_slot_{s}"))),
            white: atlas.white(),
        }
    }

    /// `nine` over `rect`, or a flat `fallback` when the pack lacks it.
    pub fn paint(&self, list: &mut DrawList, nine: Option<Nine>, rect: [f32; 4], tint: [u8; 4], fallback: u32) {
        match nine {
            Some(nine) => list.nine(nine, rect, tint),
            None => list.fill(self.white, rect, rgba(fallback, tint[3])),
        }
    }

    /// Wrapped label lines of `widget` 194 wide; empty for widgets without a label.
    fn label_lines(&self, font: Option<&Font>, widget: &Widget, width: f32) -> Vec<String> {
        let text = widget.label_with_value().filter(|_| !matches!(widget, Widget::Dropdown { .. }));
        let text = match (widget, text) {
            (_, Some(t)) => t,
            (Widget::Dropdown { label, .. } | Widget::Input { label, .. }, None) => label.clone(),
            _ => return Vec::new(),
        };
        lines(font, &text, width)
    }

    /// The row height of `widget` in a custom form `width` wide, its spacer included.
    pub fn height(&self, font: Option<&Font>, widget: &Widget, width: f32) -> f32 {
        let label = self.label_lines(font, widget, width).len() as f32 * LINE;
        match widget {
            Widget::Text { text } | Widget::Header { text } => lines(font, text, width).len() as f32 * LINE,
            Widget::Divider => 9.0,
            Widget::Button { .. } => 32.0,
            Widget::Toggle { label, .. } => (lines(font, label, width - 34.0).len() as f32 * LINE + 2.0).max(TOGGLE[1]) + SPACER,
            Widget::Slider { .. } | Widget::Steps { .. } => label + LABEL_GAP + SLIDER_HEIGHT + SPACER,
            Widget::Dropdown { .. } | Widget::Input { .. } => label + LABEL_GAP + BOX_HEIGHT + SPACER,
        }
    }

    /// The control below a row's label: the slider panel or the 30 px box.
    fn control(&self, font: Option<&Font>, widget: &Widget, rect: [f32; 4]) -> [f32; 4] {
        let [l, t, r, _] = rect;
        let top = t + self.label_lines(font, widget, r - l).len() as f32 * LINE + LABEL_GAP;
        let h = if matches!(widget, Widget::Slider { .. } | Widget::Steps { .. }) { SLIDER_HEIGHT } else { BOX_HEIGHT };
        [l, top, r, top + h]
    }
}

pub(crate) fn rgba(rgb: u32, a: u8) -> [u8; 4] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a]
}

pub(crate) fn inset(r: [f32; 4], by: f32) -> [f32; 4] {
    [r[0] + by, r[1] + by, r[2] - by, r[3] - by]
}

impl Skin for Kit {
    fn draw(&self, list: &mut DrawList, font: Option<&Font>, widget: &Widget, rect: [f32; 4], state: State) {
        draw::widget(self, list, font, widget, rect, state);
    }

    fn hit(&self, font: Option<&Font>, widget: &Widget, rect: [f32; 4]) -> [f32; 4] {
        match widget {
            Widget::Slider { .. } | Widget::Steps { .. } | Widget::Dropdown { .. } | Widget::Input { .. } => self.control(font, widget, rect),
            Widget::Toggle { .. } => [rect[0], rect[1], rect[2], rect[3] - SPACER],
            _ => rect,
        }
    }

    fn slider_span(&self, _font: Option<&Font>, rect: [f32; 4]) -> [f32; 2] {
        handle_span(rect)
    }

    fn choices_cycle(&self) -> bool {
        false
    }

    /// Below the closed control [unverified: where the engine opens it].
    fn dropdown_list(&self, _font: Option<&Font>, rect: [f32; 4], _options: usize) -> ListGeometry {
        let top = rect[3] - SPACER;
        let list = [rect[0], top, rect[2], top + LIST_HEIGHT];
        ListGeometry { rect: list, first: [list[0] + 4.0, list[1] + 3.0], row: LIST_ROW, row_width: list[2] - list[0] - 8.0 }
    }

    fn draw_dropdown_list(&self, list: &mut DrawList, font: Option<&Font>, g: ListGeometry, options: &[String], selected: usize, hover: Option<usize>, scroll: f32) {
        draw::dropdown_list(self, list, font, g, options, selected, hover, scroll);
    }

    fn scroll_track(&self, viewport: [f32; 4]) -> [f32; 4] {
        [viewport[2] + 2.0, viewport[1] + 2.0, viewport[2] + 7.0, viewport[3] - 2.0]
    }

    fn thumb(&self, track: [f32; 4], visible: f32, content: f32, scroll: f32) -> [f32; 4] {
        let h = track[3] - track[1];
        let thumb = (h * visible / content.max(visible)).clamp(10.0, h);
        let top = track[1] + scroll * (h - thumb);
        [track[0], top, track[2], top + thumb]
    }

    fn draw_scrollbar(&self, list: &mut DrawList, track: [f32; 4], thumb: [f32; 4]) {
        self.paint(list, self.rail, [track[0] + 1.0, track[1], track[2] - 1.0, track[3]], crate::draw::WHITE, 0x434343);
        self.paint(list, self.thumb, thumb, crate::draw::WHITE, 0xC6C6C6);
    }

    fn scroll_step(&self) -> f32 {
        15.0
    }
}

/// The x span of the 10 px handle's centre: the 184 px bar starts 5 px into the row.
pub(crate) fn handle_span(row: [f32; 4]) -> [f32; 2] {
    [row[0] + 10.0, row[0] + 184.0]
}
