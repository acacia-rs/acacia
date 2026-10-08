//! The Bedrock form: a fixed 225×200 dialog centred on screen, the title in its top band, a close
//! X, and one scrolling column. research/forms-bedrock-layout.md §2.1-2.4.

use super::Shape;
use crate::draw::{DrawList, WHITE};
use crate::font::Font;
use crate::widget::bedrock::{GREY, Kit, LINE};
use crate::widget::panel::{Panel, Placed};
use crate::widget::{Widget, lines};

const SIZE: [f32; 2] = [225.0, 200.0];
/// The scrolling viewport inside the dialog.
const VIEWPORT: [f32; 4] = [10.0, 23.0, 208.0, 190.0];
const BUTTON_HEIGHT: f32 = 32.0;

/// The panel, and the dialog's top-left.
pub(super) fn lay_out(kit: &Kit, font: Option<&Font>, shape: Shape, content: Vec<Widget>, buttons: Vec<Widget>, size: [f32; 2]) -> (Panel, [f32; 2]) {
    let origin = [((size[0] - SIZE[0]) / 2.0).floor(), ((size[1] - SIZE[1]) / 2.0).floor()];
    let viewport = [origin[0] + VIEWPORT[0], origin[1] + VIEWPORT[1], origin[0] + VIEWPORT[2], origin[1] + VIEWPORT[3]];
    // Action and modal forms: a 190 column 4 in (`long_form_dynamic_buttons_panel`); custom: 194, 2 in.
    let (x, width) = if shape.custom { (viewport[0] + 2.0, 194.0) } else { (viewport[0] + 4.0, 190.0) };
    let mut y = viewport[1];
    let mut items = Vec::with_capacity(content.len() + buttons.len());
    for (i, widget) in content.into_iter().enumerate() {
        let (text_at, row) = match &widget {
            // The body: at (2, 2) in a 194 column, 4 px before the buttons.
            Widget::Text { text } if shape.body && i == 0 => {
                let h = lines(font, text, 194.0).len() as f32 * LINE;
                items.push(Placed { rect: [viewport[0] + 2.0, y + 2.0, viewport[0] + 196.0, y + 2.0 + h], widget, scrolls: true });
                y += 2.0 + h + 4.0;
                continue;
            }
            Widget::Text { text } if shape.custom => (4.0, text_height(font, text, width) + 9.0),
            Widget::Header { text } if shape.custom => (4.0, text_height(font, text, width) + 5.0),
            Widget::Text { text } => (6.0, text_height(font, text, width) + 11.0),
            Widget::Header { text } => (8.0, text_height(font, text, width) + 9.0),
            Widget::Button { .. } => (0.0, BUTTON_HEIGHT),
            _ => (0.0, kit.height(font, &widget, width)),
        };
        let rect = match widget {
            Widget::Text { .. } | Widget::Header { .. } => [x, y + text_at, x + width, y + row],
            _ => [x, y, x + width, y + row],
        };
        items.push(Placed { widget, rect, scrolls: true });
        y += row;
    }
    for widget in buttons {
        items.push(Placed { widget, rect: [x, y, x + width, y + BUTTON_HEIGHT], scrolls: true });
        y += BUTTON_HEIGHT;
    }
    (Panel::new(items, viewport, 2.0), origin)
}

fn text_height(font: Option<&Font>, text: &str, width: f32) -> f32 {
    lines(font, text, width).len() as f32 * LINE
}

/// The close button: 21×21 in the top-right corner.
pub(super) fn close_rect(origin: [f32; 2]) -> [f32; 4] {
    [origin[0] + 204.0, origin[1], origin[0] + 225.0, origin[1] + 21.0]
}

pub(super) fn draw_frame(list: &mut DrawList, kit: &Kit, font: Option<&Font>, title: &str, origin: [f32; 2], close_hover: bool, close_held: bool) {
    let [x, y] = origin;
    kit.paint(list, kit.hole, [x + 4.0, y + 4.0, x + 221.0, y + 196.0], [255, 255, 255, 204], 0x090909);
    kit.paint(list, kit.dialog, [x, y, x + SIZE[0], y + SIZE[1]], WHITE, 0xC6C6C6);
    if let Some(font) = font {
        let line = lines(Some(font), title, 210.0).into_iter().next().unwrap_or_default();
        let tx = (x + 112.5 - font.width(&line) / 2.0).floor();
        font.draw(list, &line, tx, y + 10.0, GREY, 1.0, false);
    }
    let state = if close_held && close_hover { 2 } else { usize::from(close_hover) };
    kit.paint(list, kit.close[state], [x + 207.0, y + 3.0, x + 222.0, y + 18.0], WHITE, 0x8B8B8B);
}
