//! The scoreboard sidebar: an objective's title over its top fifteen lines, scores right-aligned in
//! red, at the right edge and a third below the middle (Java's `Gui.displayScoreboardSidebar`;
//! Bedrock's own placement in `scoreboards.json` is not measured, so both looks share it).

use crate::draw::DrawList;
use crate::font::LINE_HEIGHT;
use crate::theme::Theme;

/// Java shows the first fifteen lines.
const MAX_LINES: usize = 15;
const MARGIN: f32 = 3.0;
/// `ChatFormatting.RED`.
const SCORE: u32 = 0xFF5555;
/// `getBackgroundColor(0.3)` behind the lines, `0.4` behind the title.
const LINES_BACK: [u8; 4] = [0, 0, 0, 76];
const TITLE_BACK: [u8; 4] = [0, 0, 0, 102];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sidebar {
    /// With `§` codes.
    pub title: String,
    /// Name (with `§` codes) and score, in display order.
    pub lines: Vec<(String, i32)>,
}

pub fn draw(list: &mut DrawList, theme: &Theme, sidebar: &Sidebar, [w, h]: [f32; 2]) {
    let Some(font) = &theme.font else { return };
    let lines = &sidebar.lines[..sidebar.lines.len().min(MAX_LINES)];
    let colon = font.width(": ");
    let width = lines.iter().map(|(name, score)| font.width(name) + colon + font.width(&score.to_string())).fold(font.width(&sidebar.title), f32::max);
    let height = lines.len() as f32 * LINE_HEIGHT;
    let bottom = (h / 2.0 + (height / 3.0).floor()).floor();
    let left = w - width - MARGIN;
    let right = w - MARGIN + 2.0;
    let top = bottom - height;
    let white = theme.atlas.white();
    list.fill(white, [left - 2.0, top - LINE_HEIGHT - 1.0, right, top - 1.0], TITLE_BACK);
    list.fill(white, [left - 2.0, top - 1.0, right, bottom], LINES_BACK);
    font.draw(list, &sidebar.title, (left + width / 2.0 - font.width(&sidebar.title) / 2.0).floor(), top - LINE_HEIGHT, 0xFFFFFF, 1.0, false);
    for (i, (name, score)) in lines.iter().enumerate() {
        let y = top + i as f32 * LINE_HEIGHT;
        font.draw(list, name, left, y, 0xFFFFFF, 1.0, false);
        let score = score.to_string();
        font.draw(list, &score, right - font.width(&score), y, SCORE, 1.0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::test_theme::theme;

    #[test]
    fn sits_right_and_a_third_below_the_middle() {
        let theme = theme(true);
        let sidebar = Sidebar { title: "Kills".into(), lines: (0..20).map(|i| (format!("p{i}"), 20 - i)).collect() };
        let mut list = DrawList::new(1.0);
        draw(&mut list, &theme, &sidebar, [320.0, 240.0]);
        // Fifteen lines of 9: bottom at 120 + 45, the lines' backing from 29 rows up.
        let backing = list.quads[1].rect;
        assert_eq!(backing[3], 165.0);
        assert_eq!(backing[1], 165.0 - 135.0 - 1.0);
        assert_eq!(backing[2], 319.0, "two past the 3 px margin");
    }
}
