//! `ClientBundleTooltip` (`BundleItem.getTooltipImage`): a bundle's contents as a grid of slots
//! above a weight bar, and an item tooltip's components as `setTooltipForNextFrame(font, lines,
//! image, ...)` assembles them.

use serde_json::Value;

use super::game::Game;
use super::item::{self, Details, Display};
use super::stack::ItemStack;
use super::{Line, Positioner, Tooltip};
use crate::java::gui::{Gui, Metrics};
use crate::java::mth::WHITE;
use crate::java::split::split;
use crate::java::text::{Lang, Text};

/// The key of a bundle's item `i` in its slots' item calls (equal stacks share the first's).
pub const CONTENT_KEY: i32 = 10000;
const GRID_WIDTH: i32 = 96;
const SLOT_SIZE: i32 = 24;
/// `item.minecraft.bundle.empty.description`'s colour.
const DESCRIPTION: u32 = 0xFFAA_AAAA;

/// A commons-lang `Fraction`, kept reduced; Java's int overflow (an "excessive" weight) is `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fraction(i64, i64);

impl Fraction {
    const ZERO: Fraction = Fraction(0, 1);
    const ONE: Fraction = Fraction(1, 1);

    fn new(n: i64, d: i64) -> Option<Fraction> {
        let g = gcd(n.abs(), d.abs()).max(1);
        let (n, d) = (n / g, d / g);
        (n.abs() <= i64::from(i32::MAX) && d <= i64::from(i32::MAX)).then_some(Fraction(n, d))
    }

    fn add(self, o: Fraction) -> Option<Fraction> {
        Fraction::new(self.0 * o.1 + o.0 * self.1, self.1 * o.1)
    }

    fn at_least_one(self) -> bool {
        self.0 >= self.1
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `BundleContents`: its templates created, its weight, and the texts its image shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Bundle {
    /// The templates (their counts unchecked), and the stacks they create.
    templates: Vec<ItemStack>,
    items: Vec<ItemStack>,
    weight: Option<Fraction>,
    description: Vec<Text>,
    empty: Text,
    full: Text,
}

/// `BundleContents.getWeight`: a nested bundle's weight and 1/16, a beehive's 1, else one stack's share.
fn item_weight(stack: &ItemStack, game: &Game) -> Option<Fraction> {
    if let Some(contents) = stack.get("bundle_contents") {
        return content_weight(&templates(contents, game), game)?.add(Fraction(1, 16));
    }
    if stack.get("bees").and_then(Value::as_array).is_some_and(|b| !b.is_empty()) {
        return Some(Fraction::ONE);
    }
    Fraction::new(1, i64::from(stack.max_stack_size()))
}

fn content_weight(items: &[ItemStack], game: &Game) -> Option<Fraction> {
    items.iter().try_fold(Fraction::ZERO, |sum, s| {
        let w = item_weight(s, game)?;
        sum.add(Fraction::new(w.0 * i64::from(s.count), w.1)?)
    })
}

/// `BundleItem.isBarVisible`, `getBarWidth` and `getBarColor` (RGB) of a bundle.
pub fn bar(stack: &ItemStack, game: &Game) -> Option<(i32, u32)> {
    let contents = stack.get("bundle_contents")?;
    // getWeightSafe: a weight that overflows counts as none.
    let weight = content_weight(&templates(contents, game), game).unwrap_or(Fraction::ZERO);
    if weight.0 <= 0 {
        return None;
    }
    let width = (1 + (weight.0 * 12 / weight.1) as i32).min(13);
    Some((width, if weight.at_least_one() { 0xFF5454 } else { 0x7087FF }))
}

fn templates(contents: &Value, game: &Game) -> Vec<ItemStack> {
    contents.as_array().into_iter().flatten().filter_map(|t| game.template(t)).collect()
}

impl Bundle {
    fn new(contents: &Value, font: &dyn Metrics, lang: Lang, game: &Game) -> Bundle {
        let templates = templates(contents, game);
        let items = contents.as_array().into_iter().flatten().filter_map(|t| game.stack(t)).collect();
        let description = Text::translate("item.minecraft.bundle.empty.description", &[], lang);
        Bundle {
            weight: content_weight(&templates, game),
            templates,
            items,
            description: split(font, &description, GRID_WIDTH),
            empty: Text::translate("item.minecraft.bundle.empty", &[], lang),
            full: Text::translate("item.minecraft.bundle.full", &[], lang),
        }
    }

    fn grid_rows(&self) -> i32 {
        (self.items.len().min(12) as i32 + 3) / 4
    }

    fn height(&self) -> i32 {
        let content = if self.items.is_empty() { self.description.len() as i32 * 9 } else { self.grid_rows() * SLOT_SIZE };
        content + 13 + 8
    }

    /// `getNumberOfItemsToShow`: eleven when the "+N" slot is needed, the last row kept full.
    fn shown(&self) -> usize {
        let n = self.items.len();
        let available = if n > 12 { 11 } else { 12 };
        let gap = if n.is_multiple_of(4) { 0 } else { 4 - n % 4 };
        n.min(available - gap)
    }

    fn key(&self, i: usize) -> i32 {
        CONTENT_KEY + self.items.iter().position(|s| *s == self.items[i]).unwrap_or(i) as i32
    }

    /// `extractImage` at (`x`, `y`) in a tooltip `w` wide.
    pub fn extract(&self, gui: &mut Gui, font: &dyn Metrics, x: i32, y: i32, w: i32) {
        let Some(weight) = self.weight else { return };
        let left = x + (w - GRID_WIDTH) / 2;
        if self.items.is_empty() {
            for (n, line) in self.description.iter().enumerate() {
                gui.text(line.clone(), left, y + 9 * n as i32, DESCRIPTION, true);
            }
            self.progress(gui, font, left, y + self.description.len() as i32 * 9 + 4, Fraction::ZERO);
            return;
        }
        let shown = self.shown();
        let (right, bottom) = (left + GRID_WIDTH, y + self.grid_rows() * SLOT_SIZE);
        let mut slot = 1;
        for row in 1..=self.grid_rows() {
            for column in 1..=4 {
                let (dx, dy) = (right - column * SLOT_SIZE, bottom - row * SLOT_SIZE);
                if self.items.len() > 12 && column * row == 1 {
                    let hidden: i32 = self.templates[shown..].iter().map(|s| s.count).sum();
                    let text = Text::literal(format!("+{hidden}"));
                    gui.centered(text.clone(), font.width(&text), dx + 12, dy + 10, WHITE);
                } else if shown >= slot {
                    let key = self.key(shown - slot);
                    // TODO: the selected item's highlight and name tooltip (no selection is kept yet).
                    gui.sprite("container/bundle/slot_background", dx, dy, SLOT_SIZE, SLOT_SIZE);
                    if !self.items[shown - slot].is_empty() {
                        gui.item(key, dx + 4, dy + 4, slot as i32);
                        gui.decorations(key, dx + 4, dy + 4, None);
                    }
                    slot += 1;
                }
            }
        }
        self.progress(gui, font, left, bottom + 4, weight);
    }

    fn progress(&self, gui: &mut Gui, font: &dyn Metrics, x: i32, y: i32, weight: Fraction) {
        let fill = (weight.0 * 94 / weight.1).clamp(0, 94) as i32;
        let bar = if weight.at_least_one() { "full" } else { "fill" };
        gui.sprite(format!("container/bundle/bundle_progressbar_{bar}"), x + 1, y, fill, 13);
        gui.sprite("container/bundle/bundle_progressbar_border", x, y, GRID_WIDTH, 13);
        let text = if weight == Fraction::ZERO { Some(&self.empty) } else { weight.at_least_one().then_some(&self.full) };
        if let Some(t) = text {
            gui.centered(t.clone(), font.width(t), x + 48, y + 3, WHITE);
        }
    }
}

/// The bundle image of a bundle item whose `bundle_contents` show.
pub fn image(stack: &ItemStack, display: &Display, font: &dyn Metrics, lang: Lang, game: &Game) -> Option<Line> {
    if !stack.id.ends_with("bundle") || !display.shows("bundle_contents") {
        return None;
    }
    let bundle = Bundle::new(stack.get("bundle_contents")?, font, lang, game);
    Some(Line::Image { kind: "ClientBundleTooltip".into(), w: GRID_WIDTH, h: bundle.height(), bundle: Some(Box::new(bundle)) })
}

/// The tooltip a container screen sets for a hovered stack (`extractTooltip`): its lines, its
/// image second, its `tooltip_style`, extra space after the name; at the mouse.
pub fn stack_tooltip(stack: &ItemStack, d: &Details, font: &dyn Metrics, mouse: [i32; 2]) -> Tooltip {
    let mut lines: Vec<Line> = item::tooltip_lines(stack, d).into_iter().map(Line::Text).collect();
    if let Some(image) = image(stack, &Display::of(stack), font, d.lang, d.game) {
        lines.insert(lines.len().min(1), image);
    }
    let style = stack.get("tooltip_style").and_then(Value::as_str).map(str::to_owned);
    Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style, extra_space: true }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::java::gui::{Fixed, GuiDraw};

    #[test]
    fn thirteen_stacks_show_eight_a_count_and_their_weight() {
        let game = Game::new(json!({"item": {"minecraft:stone": {"components": {"minecraft:max_stack_size": 64}}}}));
        let contents = json!(vec![json!({"id": "minecraft:stone", "count": 1}); 13]);
        let bundle = Bundle::new(&contents, &Fixed(6.0), &|_: &str| None, &game);
        assert_eq!((bundle.shown(), bundle.weight, bundle.height()), (8, Some(Fraction(13, 64)), 3 * 24 + 21));
        let mut gui = Gui::new(200, 200);
        bundle.extract(&mut gui, &Fixed(6.0), 10, 20, 96);
        let items: Vec<i32> = gui.draws.iter().filter_map(|d| if let GuiDraw::Item { key, .. } = d { Some(*key) } else { None }).collect();
        assert_eq!(items, vec![CONTENT_KEY; 8]);
        assert!(gui.draws.iter().any(|d| matches!(d, GuiDraw::Text { text, x: 88, y: 78, .. } if text.plain() == "+5")));
        let fill = gui.draws.iter().find_map(|d| if let GuiDraw::Sprite { sprite, w, .. } = d { sprite.ends_with("_fill").then_some(*w) } else { None });
        assert_eq!(fill, Some(13 * 94 / 64));
    }
}
