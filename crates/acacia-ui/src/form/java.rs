//! The Java form, laid out as Java's server dialogs (`DialogScreen`): a 33 px header with the title,
//! a centred 200 px column 10 apart that scrolls, the answer buttons in a 33 px footer, the world
//! dimmed behind. research/forms-java-widgets.md §2h, §3.

use crate::atlas::Sprite;
use crate::draw::DrawList;
use crate::font::{Font, LINE_HEIGHT};
use crate::widget::java::{HEIGHT, Kit};
use crate::widget::panel::{Panel, Placed};
use crate::widget::Widget;

const HEADER: f32 = 33.0;
const COLUMN: f32 = 200.0;
const SPACING: f32 = 10.0;
/// Between stacked buttons (`packControlsIntoColumns`).
const BUTTON_SPACING: f32 = 2.0;
/// Footer buttons: 150 wide, 8 apart.
const FOOTER_BUTTON: f32 = 150.0;
const FOOTER_SPACING: f32 = 8.0;
/// Scrollbar room on each side of the column (`ScrollableLayout`).
const SCROLL_ROOM: f32 = 10.0;

pub(super) fn lay_out(kit: &Kit, font: Option<&Font>, content: Vec<Widget>, buttons: Vec<Widget>, size: [f32; 2]) -> Panel {
    let [w, h] = size;
    let footer = if buttons.is_empty() { 5.0 } else { HEADER };
    let left = (w / 2.0).floor() - COLUMN / 2.0;
    let mut y = 0.0;
    let mut rows = Vec::with_capacity(content.len());
    let mut previous_button = false;
    for widget in content {
        let is_button = matches!(widget, Widget::Button { .. });
        if !rows.is_empty() {
            y += if is_button && previous_button { BUTTON_SPACING } else { SPACING };
        }
        let height = kit.height(font, &widget, COLUMN);
        rows.push((widget, y, height));
        y += height;
        previous_button = is_button;
    }
    let content_height = y;
    let room = (h - HEADER - footer).max(0.0);
    let visible = content_height.min(room);
    let top = (h - footer - visible).min(HEADER + 30.0).max(HEADER).floor();
    let viewport = [left - SCROLL_ROOM, top, left + COLUMN + SCROLL_ROOM, top + visible];
    let mut items: Vec<Placed> = rows.into_iter().map(|(widget, y, height)| Placed { widget, rect: [left, top + y, left + COLUMN, top + y + height], scrolls: true }).collect();
    let n = buttons.len() as f32;
    let row_width = n * FOOTER_BUTTON + (n - 1.0).max(0.0) * FOOTER_SPACING;
    let by = h - footer + ((footer - HEIGHT) / 2.0).round();
    for (i, widget) in buttons.into_iter().enumerate() {
        let x = ((w - row_width) / 2.0).floor() + i as f32 * (FOOTER_BUTTON + FOOTER_SPACING);
        items.push(Placed { widget, rect: [x, by, x + FOOTER_BUTTON, by + HEIGHT], scrolls: false });
    }
    Panel::new(items, viewport, 0.0)
}

/// The in-world dim (`inworld_menu_background`, black at 0x40; Java also blurs) and the title.
pub(super) fn draw_frame(list: &mut DrawList, white: Sprite, font: Option<&Font>, title: &str, size: [f32; 2]) {
    list.fill(white, [0.0, 0.0, size[0], size[1]], [0, 0, 0, 0x40]);
    if let Some(font) = font {
        let x = ((size[0] - font.width(title)) / 2.0).floor();
        font.draw(list, title, x, ((HEADER - LINE_HEIGHT) / 2.0).round(), 0xFFFFFF, 1.0, true);
    }
}
