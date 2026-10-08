//! Drawing Bedrock's controls; geometry in the parent module.

use super::{BORDER, DARK, GREY, Kit, LINE, PLACEHOLDER, TOGGLE, WHITE, handle_span, inset, rgba};
use crate::draw::{DrawList, WHITE as OPAQUE};
use crate::font::Font;
use crate::widget::{ListGeometry, State, Widget, clipped, draw_lines, lines};

pub(super) fn widget(kit: &Kit, list: &mut DrawList, font: Option<&Font>, widget: &Widget, rect: [f32; 4], state: State) {
    let [l, t, r, _] = rect;
    let label = kit.label_lines(font, widget, r - l);
    draw_lines(list, font, &label, l, t, LINE, false, WHITE, false);
    match widget {
        Widget::Text { text } => draw_lines(list, font, &lines(font, text, r - l), l, t, LINE, false, WHITE, false),
        Widget::Header { text } => draw_lines(list, font, &lines(font, &format!("§l{text}"), r - l), l, t, LINE, false, WHITE, false),
        Widget::Divider => list.fill(kit.white, [l, t + 4.0, r, t + 5.0], rgba(0xC6C6C6, 255)),
        Widget::Button { text, image } => button(kit, list, font, text, *image, rect, state),
        Widget::Toggle { label, on } => {
            let image = kit.toggle[usize::from(*on) * 2 + usize::from(state.hover)];
            kit.paint(list, image, [l, t, l + TOGGLE[0], t + TOGGLE[1]], OPAQUE, if *on { 0x218306 } else { 0x8B8B8B });
            draw_lines(list, font, &lines(font, label, r - l - 34.0), l + 34.0, t + 3.0, LINE, false, WHITE, false);
        }
        Widget::Slider { min, max, value, .. } => {
            let at = if max > min { ((value - min) / (max - min)) as f32 } else { 0.0 };
            slider(kit, list, kit.control(font, widget, rect), at, state);
        }
        Widget::Steps { options, index, .. } => {
            let at = if options.len() > 1 { *index as f32 / (options.len() - 1) as f32 } else { 0.0 };
            slider(kit, list, kit.control(font, widget, rect), at, state);
        }
        Widget::Dropdown { options, index, .. } => {
            let control = kit.control(font, widget, rect);
            dropdown(kit, list, font, options.get(*index).map_or("", String::as_str), control, state);
        }
        Widget::Input { placeholder, edit, .. } => {
            let control = kit.control(font, widget, rect);
            kit.paint(list, kit.edit[usize::from(state.hover || state.focus)], control, OPAQUE, 0x5E5E5E);
            let area = [control[0] + 3.0, control[1] + 2.0, control[2] - 3.0, control[3] - 2.0];
            let y = ((area[1] + area[3]) / 2.0 - 4.0).floor();
            let Some(font) = font else { return };
            clipped(list, area, |list| {
                if edit.is_empty() && !state.focus {
                    font.draw(list, placeholder, area[0], y, if state.hover { WHITE } else { PLACEHOLDER }, 1.0, false);
                    return;
                }
                let caret = font.width(&edit.before_caret());
                let x = area[0] - (caret + 2.0 - (area[2] - area[0])).max(0.0);
                font.draw(list, &edit.text(), x, y, WHITE, 1.0, false);
                if state.caret {
                    list.fill(kit.white, [x + caret, y - 1.0, x + caret + 1.0, y + 9.0], rgba(WHITE, 255));
                }
            });
        }
    }
}

/// A light button over `rect`: an image column first when it has one, text centred, two lines.
fn button(kit: &Kit, list: &mut DrawList, font: Option<&Font>, text: &str, image: Option<crate::atlas::Sprite>, rect: [f32; 4], state: State) {
    let [l, t, r, b] = rect;
    let left = match image {
        Some(sprite) => {
            let y = ((t + b) / 2.0 - 16.0).floor();
            list.sprite_stretched(sprite, [l + 1.0, y, l + 33.0, y + 32.0], OPAQUE);
            l + 34.0
        }
        None => l,
    };
    let lit = state.hover || state.pressed || state.focus;
    let face = [l.max(left), t, r, b];
    kit.paint(list, kit.border, face, if lit { OPAQUE } else { BORDER }, 0x131313);
    let texture = if state.pressed { 2 } else { usize::from(lit) };
    kit.paint(list, kit.button[texture], inset(face, 1.0), OPAQUE, if lit { 0x218306 } else { 0xC6C6C6 });
    let mut wrapped = lines(font, text, face[2] - face[0] - 6.0);
    wrapped.truncate(2);
    let y = ((t + b) / 2.0 - wrapped.len() as f32 * LINE / 2.0).floor() + 1.0 + f32::from(u8::from(state.pressed));
    draw_lines(list, font, &wrapped, (face[0] + face[2]) / 2.0, y, LINE, true, if lit { WHITE } else { GREY }, false);
}

/// The 184×10 bar with its fill to `at` (0 to 1), and the 10×16 handle on it.
fn slider(kit: &Kit, list: &mut DrawList, control: [f32; 4], at: f32, state: State) {
    let [l, t, _, _] = control;
    let bar = [l + 5.0, t + 3.0, l + 189.0, t + 13.0];
    let lit = state.hover || state.focus;
    kit.paint(list, kit.slider_border, bar, if lit { OPAQUE } else { [0, 0, 0, 255] }, if lit { WHITE } else { 0 });
    let inner = inset(bar, 1.0);
    kit.paint(list, kit.slider_back[usize::from(lit)], inner, OPAQUE, if lit { 0x037300 } else { 0x404040 });
    let filled = [inner[0], inner[1], inner[0] + (inner[2] - inner[0]) * at.clamp(0.0, 1.0), inner[3]];
    if filled[2] > filled[0] {
        kit.paint(list, kit.slider_fill[usize::from(lit)], filled, OPAQUE, if lit { 0x4E8836 } else { 0x808080 });
    }
    let [a, z] = handle_span(control);
    let x = (a + (z - a) * at.clamp(0.0, 1.0) - 5.0).round();
    let handle = if state.pressed { 2 } else { usize::from(state.hover) };
    kit.paint(list, kit.handle[handle], [x, t, x + 10.0, t + 16.0], OPAQUE, 0xC6C6C6);
}

/// The closed dropdown: the chosen option and a chevron in a light toggle button.
fn dropdown(kit: &Kit, list: &mut DrawList, font: Option<&Font>, text: &str, control: [f32; 4], state: State) {
    let lit = state.hover || state.focus;
    kit.paint(list, kit.border, control, if lit { OPAQUE } else { BORDER }, 0x131313);
    let texture = match (state.open, lit) {
        (false, lit) => usize::from(lit),
        (true, true) => 2,
        (true, false) => 3,
    };
    kit.paint(list, kit.button[texture], inset(control, 1.0), OPAQUE, 0xC6C6C6);
    let colour = if lit { WHITE } else if state.open { DARK } else { GREY };
    let y = ((control[1] + control[3]) / 2.0 - 4.0).floor();
    if let Some(font) = font {
        clipped(list, [control[0] + 11.0, control[1], control[2] - 27.0, control[3]], |list| {
            font.draw(list, text, control[0] + 11.0, y, colour, 1.0, false);
        });
    }
    let chevron = kit.chevron[usize::from(lit && !state.open)];
    let cx = control[2] - 19.0;
    kit.paint(list, chevron, [cx, y, cx + 8.0, y + 8.0], OPAQUE, colour);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn dropdown_list(kit: &Kit, list: &mut DrawList, font: Option<&Font>, g: ListGeometry, options: &[String], selected: usize, hover: Option<usize>, scroll: f32) {
    kit.paint(list, kit.list_back, g.rect, OPAQUE, 0x313131);
    clipped(list, inset(g.rect, 2.0), |list| {
        for (i, option) in options.iter().enumerate() {
            let y = g.first[1] + i as f32 * g.row - scroll;
            let row = [g.first[0] - 2.0, y - 1.0, g.first[0] + g.row_width + 2.0, y + g.row - 1.0];
            if hover == Some(i) {
                list.fill(kit.white, row, rgba(0x177400, 255));
            } else if i == selected {
                list.fill(kit.white, row, rgba(0x4A484A, 255));
            }
            let radio = kit.radio[usize::from(i == selected) * 2 + usize::from(hover == Some(i))];
            let ry = y + 3.5;
            kit.paint(list, radio, [g.first[0] + 3.0, ry, g.first[0] + 13.0, ry + 10.0], OPAQUE, 0xC6C6C6);
            if let Some(font) = font {
                font.draw(list, option, g.first[0] + 19.0, y + 4.0, WHITE, 1.0, false);
            }
        }
    });
}
