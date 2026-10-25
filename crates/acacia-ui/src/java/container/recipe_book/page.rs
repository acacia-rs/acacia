//! `RecipeBookPage`: twenty `RecipeButton`s over the listed collections, the page number and
//! arrows, and the `OverlayRecipeComponent` of a button's alternatives.

use super::{Book, Collection, Entry, IMAGE_HEIGHT, IMAGE_WIDTH, widgets};
use crate::java::container::{Env, same_item};
use crate::java::gui::{Gui, PoseOp};
use crate::java::mth::{WHITE, ceil};
use crate::java::text::Text;
use crate::java::tooltip::Deferred;

pub const PER_PAGE: usize = 20;

/// `(listed collections, current page, total pages)`.
pub fn layout(book: &Book) -> (Vec<usize>, usize, usize) {
    let listed = book.listed();
    let total = listed.len().div_ceil(PER_PAGE);
    (listed, book.page.min(total.saturating_sub(1)), total)
}

/// The collection under page button `b`.
fn collection(book: &Book, b: usize) -> Option<&Collection> {
    let (listed, page, _) = layout(book);
    listed.get(page * PER_PAGE + b).map(|&c| &book.collections[c])
}

/// `RecipeButton.getDisplayStack` of page button `b` (its key, `None` when empty) and whether it
/// holds several recipes.
pub fn display(book: &Book, b: usize) -> (Option<i32>, bool) {
    let Some(c) = collection(book, b) else { return (None, false) };
    let fitting = c.fitting(book.filtering);
    let n = fitting.len().max(1);
    let index = book.index();
    let (offset, entry) = (index / n, index % n);
    let key = fitting.get(entry).and_then(|e| (!e.display.is_empty()).then(|| e.display[offset % e.display.len()]));
    (key, fitting.len() > 1)
}

/// `allRecipesHaveSameResultDisplay`.
fn same_display(book: &Book, fitting: &[&Entry]) -> bool {
    let mut keys = fitting.iter().flat_map(|e| e.display.iter());
    let Some(first) = keys.next().and_then(|k| book.items.get(k)) else { return true };
    keys.all(|k| book.items.get(k).is_some_and(|s| same_item(first, s)))
}

/// `1 + 0.1 sin(t / 15 π)`: the squeeze of a tab or button showing new recipes.
pub fn squeeze(time: f32) -> f32 {
    1.0 + 0.1 * f64::from(time / 15.0 * std::f32::consts::PI).sin() as f32
}

/// `RecipeBookPage.extractRenderState`; the hovered button goes to `book.hovered`.
pub fn render(book: &mut Book, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32, tooltip: &mut Deferred) {
    let [xo, yo] = book.origin();
    let (listed, page, total) = layout(book);
    if total > 1 {
        let args = [Text::literal((page + 1).to_string()), Text::literal(total.to_string())];
        let text = Text::translate("gui.recipebook.page", &args, env.lang);
        let w = env.font.width(&text);
        gui.text(text, xo - w / 2 + 73, yo + 141, WHITE, true);
    }
    book.hovered = None;
    let shown = listed.len().saturating_sub(page * PER_PAGE).min(PER_PAGE);
    for i in 0..shown {
        let (x, y) = (xo + 11 + 25 * (i as i32 % 5), yo + 31 + 25 * (i as i32 / 5));
        button(book, gui, i, x, y, partial);
        if widgets::hovered([x, y, 25, 25], mouse) {
            book.hovered = Some(i);
        }
    }
    let arrows = [
        (total > 1 && page + 1 < total, xo + 93, "recipe_book/page_forward", "gui.recipebook.next_page"),
        (total > 1 && page > 0, xo + 38, "recipe_book/page_backward", "gui.recipebook.previous_page"),
    ];
    for (visible, x, sprite, key) in arrows {
        if visible {
            let rect = [x, yo + 137, 12, 17];
            let hovered = widgets::hovered(rect, mouse);
            gui.sprite(if hovered { format!("{sprite}_highlighted") } else { sprite.to_owned() }, rect[0], rect[1], rect[2], rect[3]);
            if hovered {
                widgets::tooltip(tooltip, env, key, rect, mouse);
            }
        }
    }
    gui.next_stratum();
    if let Some(b) = book.overlay.filter(|&b| b < shown) {
        overlay(book, gui, mouse, b, [xo + 11 + 25 * (b as i32 % 5), yo + 31 + 25 * (b as i32 / 5)]);
    }
}

/// `RecipeButton.extractWidgetRenderState` of page button `i` at `(x, y)`.
fn button(book: &mut Book, gui: &mut Gui, i: usize, x: i32, y: i32, partial: f32) {
    let Some(c) = collection(book, i) else { return };
    let fitting = c.fitting(book.filtering);
    let many = fitting.len() > 1;
    let sprite = match (c.has_craftable(), many) {
        (true, true) => "recipe_book/slot_many_craftable",
        (true, false) => "recipe_book/slot_craftable",
        (false, true) => "recipe_book/slot_many_uncraftable",
        (false, false) => "recipe_book/slot_uncraftable",
    };
    let same = same_display(book, &fitting);
    drop(fitting);
    let anim = book.button_anims.get(i).copied().unwrap_or(0.0);
    if anim > 0.0 {
        let s = squeeze(anim);
        let (cx, cy) = ((x + 8) as f32, (y + 12) as f32);
        gui.push(vec![PoseOp::Translate(cx, cy), PoseOp::Scale(s, s), PoseOp::Translate(-cx, -cy)]);
        book.button_anims[i] = anim - partial;
    }
    gui.sprite(sprite, x, y, 25, 25);
    let (key, _) = display(book, i);
    let mut offset = 4;
    if let Some(key) = key {
        if many && same {
            gui.item(key, x + offset + 1, y + offset + 1, 0);
            offset -= 1;
        }
        gui.item(key, x + offset, y + offset, 0);
    }
    if anim > 0.0 {
        gui.pop();
    }
}

/// `OverlayRecipeComponent.init`'s corner for `total` buttons opened from the page button at `(bx, by)`.
fn overlay_origin(book: &Book, bx: i32, by: i32, total: i32) -> [i32; 2] {
    let [xo, yo] = book.origin();
    let (cx, cy) = (xo + IMAGE_WIDTH / 2, yo + 13 + IMAGE_HEIGHT / 2);
    let max_row = if total <= 16 { 4 } else { 5 };
    let rows = (total as f32 / max_row as f32).ceil() as i32;
    let width = 25.0f32;
    let (mut x, mut y) = (bx, by);
    let right = (x + total.min(max_row) * 25) as f32;
    let max_left = (cx + 50) as f32;
    if right > max_left {
        x = (x as f32 - width * ((right - max_left) / width) as i32 as f32) as i32;
    }
    let bottom = (y + rows * 25) as f32;
    let max_bottom = (cy + 50) as f32;
    if bottom > max_bottom {
        y = (y as f32 - width * ceil((bottom - max_bottom) / width) as f32) as i32;
    }
    let top = y as f32;
    let max_top = (cy - 100) as f32;
    if top < max_top {
        y = (y as f32 - width * ceil((top - max_top) / width) as f32) as i32;
    }
    [x, y]
}

/// `OverlayRecipeComponent.extractRenderState` of page button `b` at `at`: craftable recipes first.
fn overlay(book: &Book, gui: &mut Gui, mouse: [i32; 2], b: usize, at: [i32; 2]) {
    let Some(c) = collection(book, b) else { return };
    let mut recipes: Vec<(&Entry, bool)> = c.entries.iter().filter(|e| e.craftable).map(|e| (e, true)).collect();
    if !book.filtering {
        recipes.extend(c.entries.iter().filter(|e| e.selected && !e.craftable).map(|e| (e, false)));
    }
    let total = recipes.len() as i32;
    let max_row = if total <= 16 { 4 } else { 5 };
    let [x, y] = overlay_origin(book, at[0], at[1], total);
    let rows = ceil(total as f32 / max_row as f32);
    gui.sprite("recipe_book/overlay_recipe", x, y, total.min(max_row) * 25 + 8, rows * 25 + 8);
    let family = if book.kind.furnace() { "furnace" } else { "crafting" };
    let index = book.index();
    for (i, (entry, craftable)) in recipes.into_iter().enumerate() {
        let i = i as i32;
        let (bx, by) = (x + 4 + 25 * (i % max_row), y + 5 + 25 * (i / max_row));
        let hovered = widgets::hovered([bx, by, 24, 24], mouse);
        let sprite = format!(
            "recipe_book/{family}_overlay{}{}",
            if craftable { "" } else { "_disabled" },
            if hovered { "_highlighted" } else { "" }
        );
        gui.sprite(sprite, bx, by, 24, 24);
        let (gx, gy) = ((bx + 2) as f32, (by + 2) as f32);
        for cell in entry.cells.iter().filter(|c| !c.items.is_empty()) {
            gui.push(vec![PoseOp::Translate(gx + cell.x as f32, gy + cell.y as f32), PoseOp::Scale(0.375, 0.375), PoseOp::Translate(-8.0, -8.0)]);
            gui.item(cell.items[index % cell.items.len()], 0, 0, 0);
            gui.pop();
        }
    }
}
