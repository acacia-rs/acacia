//! Button menus (pause, options) on the theme's widgets. Java: a title over a column of 200×20
//! buttons a quarter down, as its screens lay them out. Bedrock: the pause screen's left column,
//! light buttons 28 tall and 3 apart under the game's logo, over a dimmed left band
//! (research/pause-bedrock-layout.md in the workspace).

use crate::draw::DrawList;
use crate::theme::Theme;
use crate::widget::{State, Widget, Widgets, contains};

/// The logo's atlas name (Bedrock's `textures/ui/title`).
pub const LOGO: &str = "ui/title";

const JAVA: Column = Column { width: 200.0, height: 20.0, gap: 4.0 };
const BEDROCK: Column = Column { width: 0.0, height: 28.0, gap: 3.0 };
/// Bedrock's column: `0.3729·W − 10` wide, centred at `0.294·W`; the dim band ends at `0.5565·W`.
const BEDROCK_WIDTH: f32 = 0.3729;
const BEDROCK_CENTRE: f32 = 0.294;
const BEDROCK_BAND: f32 = 0.5565;
/// Gap under the logo, which is a fifth as tall as the column is wide.
const LOGO_GAP: f32 = 4.0;

#[derive(Debug, Clone, Copy)]
struct Column {
    width: f32,
    height: f32,
    gap: f32,
}

/// Where the heading goes and each button's rect, for a screen `size` GUI pixels big. An empty
/// title in the Bedrock look shows the logo instead.
fn layout(theme: &Theme, title: &str, count: usize, [w, h]: [f32; 2]) -> ([f32; 4], Vec<[f32; 4]>) {
    let n = count as f32;
    match theme.widgets {
        Widgets::Java(_) => {
            let x = ((w - JAVA.width) / 2.0).floor();
            let top = (h / 4.0 + 8.0).floor();
            let buttons = (0..count).map(|i| [x, top + i as f32 * (JAVA.height + JAVA.gap), x + JAVA.width, top + i as f32 * (JAVA.height + JAVA.gap) + JAVA.height]).collect();
            ([x, (h / 4.0 - 12.0).floor(), x + JAVA.width, (h / 4.0 - 3.0).floor()], buttons)
        }
        Widgets::Bedrock(_) => {
            let width = (BEDROCK_WIDTH * w - 10.0).floor();
            let x = (BEDROCK_CENTRE * w - width / 2.0).floor();
            let heading = if title.is_empty() && theme.atlas.get(LOGO).is_some() { (width * 0.2).floor() } else { crate::font::LINE_HEIGHT };
            let total = heading + LOGO_GAP + n * BEDROCK.height + (n - 1.0).max(0.0) * BEDROCK.gap;
            let top = ((h - total) / 2.0).floor();
            let first = top + heading + LOGO_GAP;
            let step = BEDROCK.height + BEDROCK.gap;
            let buttons = (0..count).map(|i| [x, first + i as f32 * step, x + width, first + i as f32 * step + BEDROCK.height]).collect();
            ([x, top, x + width, top + heading], buttons)
        }
    }
}

/// The button under `mouse`, if any.
pub fn hit(theme: &Theme, title: &str, count: usize, size: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    layout(theme, title, count, size).1.iter().position(|&r| contains(r, mouse))
}

pub fn draw(list: &mut DrawList, theme: &Theme, title: &str, buttons: &[String], mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    let font = theme.font.as_ref();
    let (heading, rects) = layout(theme, title, buttons.len(), size);
    match theme.widgets {
        Widgets::Java(_) => list.fill(white, [0.0, 0.0, size[0], size[1]], [0, 0, 0, 0x40]),
        Widgets::Bedrock(_) => {
            list.fill(white, [0.0, 0.0, size[0], size[1]], [0, 0, 0, 26]);
            list.fill(white, [0.0, 0.0, (BEDROCK_BAND * size[0]).floor(), size[1]], [0, 0, 0, 128]);
        }
    }
    match theme.atlas.get(LOGO).filter(|_| title.is_empty()) {
        Some(logo) => list.sprite_stretched(logo, heading, crate::draw::WHITE),
        None => {
            if let Some(font) = font {
                let x = ((heading[0] + heading[2]) / 2.0 - font.width(title) / 2.0).floor();
                font.draw(list, title, x, heading[1], 0xFFFFFF, 1.0, true);
            }
        }
    }
    let skin = theme.widgets.skin();
    for (label, rect) in buttons.iter().zip(rects) {
        let hover = contains(rect, mouse);
        skin.draw(list, font, &Widget::Button { text: label.clone(), image: None }, rect, State { hover, ..State::default() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::test_theme::theme;

    #[test]
    fn java_buttons_stack_from_a_quarter_down() {
        let (theme, size) = (theme(true), [320.0, 240.0]);
        assert_eq!(layout(&theme, "Game Menu", 3, size).1[0], [60.0, 68.0, 260.0, 88.0]);
        assert_eq!(hit(&theme, "Game Menu", 3, size, [61.0, 68.0 + 24.0 * 2.0 + 1.0]), Some(2));
        assert_eq!(hit(&theme, "Game Menu", 3, size, [61.0, 89.0]), None, "between buttons");
    }

    #[test]
    fn bedrock_column_sits_left_of_centre() {
        let (theme, size) = (theme(false), [376.0, 250.0]);
        let (_, rects) = layout(&theme, "Options", 3, size);
        // 0.3729·376 − 10 = 130 wide, centred at 110.
        assert_eq!(rects[0][0], 45.0);
        assert_eq!(rects[0][2] - rects[0][0], 130.0);
        assert_eq!(rects[1][1] - rects[0][1], 31.0, "28 tall, 3 apart");
    }
}
