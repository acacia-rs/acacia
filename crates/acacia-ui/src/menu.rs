//! Button menus (pause, options): a title over a column of 200×20 buttons, as Java's screens lay
//! out their widgets, drawn flat in vanilla's greys; the hovered button gets a white border.

use crate::draw::DrawList;
use crate::theme::Theme;

const WIDTH: f32 = 200.0;
const HEIGHT: f32 = 20.0;
const STEP: f32 = 24.0;

/// Top-left of button `i` for a screen `size` GUI pixels big: the column starts a quarter down.
fn button(i: usize, [w, h]: [f32; 2]) -> [f32; 2] {
    [((w - WIDTH) / 2.0).floor(), (h / 4.0 + 8.0 + i as f32 * STEP).floor()]
}

/// The button under `mouse`, if any.
pub fn hit(count: usize, size: [f32; 2], mouse: [f32; 2]) -> Option<usize> {
    (0..count).find(|&i| {
        let [x, y] = button(i, size);
        (x..x + WIDTH).contains(&mouse[0]) && (y..y + HEIGHT).contains(&mouse[1])
    })
}

const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
const FACE: [u8; 4] = [0x6F, 0x6F, 0x6F, 0xFF];
const LIGHT: [u8; 4] = [0xA8, 0xA8, 0xA8, 0xFF];
const DARK: [u8; 4] = [0x2E, 0x2E, 0x2E, 0xFF];
const EDGE: [u8; 4] = [0, 0, 0, 0xFF];
const HOVER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

pub fn draw(list: &mut DrawList, theme: &Theme, title: &str, buttons: &[String], mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    list.fill(white, [0.0, 0.0, size[0], size[1]], SHADE);
    let hovered = hit(buttons.len(), size, mouse);
    if let Some(font) = &theme.font {
        font.draw(list, title, (size[0] / 2.0 - font.width(title) / 2.0).floor(), (size[1] / 4.0 - 12.0).floor(), 0xFFFFFF, 1.0, true);
    }
    for (i, label) in buttons.iter().enumerate() {
        let [x, y] = button(i, size);
        let edge = if hovered == Some(i) { HOVER } else { EDGE };
        list.fill(white, [x, y, x + WIDTH, y + HEIGHT], edge);
        list.fill(white, [x + 1.0, y + 1.0, x + WIDTH - 1.0, y + HEIGHT - 1.0], DARK);
        list.fill(white, [x + 1.0, y + 1.0, x + WIDTH - 2.0, y + HEIGHT - 2.0], LIGHT);
        list.fill(white, [x + 2.0, y + 2.0, x + WIDTH - 2.0, y + HEIGHT - 2.0], FACE);
        if let Some(font) = &theme.font {
            let colour = if hovered == Some(i) { 0xFFFFA0 } else { 0xFFFFFF };
            font.draw(list, label, (x + WIDTH / 2.0 - font.width(label) / 2.0).floor(), y + 6.0, colour, 1.0, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_stack_from_a_quarter_down() {
        let size = [320.0, 240.0];
        assert_eq!(button(0, size), [60.0, 68.0]);
        assert_eq!(hit(3, size, [61.0, 69.0]), Some(0));
        assert_eq!(hit(3, size, [61.0, 68.0 + 24.0 * 2.0 + 1.0]), Some(2));
        assert_eq!(hit(3, size, [61.0, 89.0]), None, "between buttons");
        assert_eq!(hit(3, size, [10.0, 69.0]), None);
    }
}
