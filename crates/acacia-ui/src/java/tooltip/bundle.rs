//! `ClientBundleTooltip`'s size (`BundleItem.getTooltipImage`), and an item tooltip's components
//! as `setTooltipForNextFrame(font, lines, image, ...)` assembles them.

use serde_json::Value;

use super::item::{self, Details, Display};
use super::stack::ItemStack;
use super::{Line, Positioner, Tooltip};
use crate::java::gui::Metrics;
use crate::java::text::{Style, Text};

/// `StringSplitter.splitLines(text, width).size()` for an unstyled string: greedy, breaking at
/// the last space once a line is wider than `width`.
fn line_count(text: &str, width: f32, font: &dyn Metrics) -> i32 {
    let chars: Vec<char> = text.chars().collect();
    let style = Style::default();
    let (mut lines, mut start) = (0, 0);
    while start < chars.len() {
        let (mut w, mut space, mut next) = (0.0, None, chars.len());
        for (i, &c) in chars.iter().enumerate().skip(start) {
            if c == '\n' {
                next = i + 1;
                break;
            }
            if c == ' ' {
                space = Some(i);
            }
            w += font.advance(c, &style);
            if w > width {
                next = space.map_or(i.max(start + 1), |s| s + 1);
                break;
            }
        }
        lines += 1;
        start = next;
    }
    lines
}

/// The bundle image of a bundle item whose `bundle_contents` show: 96 wide; a row of 24 per four
/// stacks (twelve at most), or the empty description, above the 13-high progress bar.
pub fn image(stack: &ItemStack, display: &Display, font: &dyn Metrics, lang: crate::java::text::Lang) -> Option<Line> {
    if !stack.id.ends_with("bundle") || !display.shows("bundle_contents") {
        return None;
    }
    let contents = stack.get("bundle_contents")?.as_array()?;
    let h = if contents.is_empty() {
        let description = Text::translate("item.minecraft.bundle.empty.description", &[], lang).plain();
        line_count(&description, 96.0, font) * 9 + 13 + 8
    } else {
        (contents.len().min(12) as i32 + 3) / 4 * 24 + 13 + 8
    };
    Some(Line::Image { kind: "ClientBundleTooltip".into(), w: 96, h })
}

/// The tooltip a container screen sets for a hovered stack (`extractTooltip`): its lines, its
/// image second, its `tooltip_style`, extra space after the name; at the mouse.
pub fn stack_tooltip(stack: &ItemStack, d: &Details, font: &dyn Metrics, mouse: [i32; 2]) -> Tooltip {
    let mut lines: Vec<Line> = item::tooltip_lines(stack, d).into_iter().map(Line::Text).collect();
    if let Some(image) = image(stack, &Display::of(stack), font, d.lang) {
        lines.insert(lines.len().min(1), image);
    }
    let style = stack.get("tooltip_style").and_then(Value::as_str).map(str::to_owned);
    Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style, extra_space: true }
}
