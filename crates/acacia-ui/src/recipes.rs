//! The recipe book beside an inventory screen (Java's `RecipeBookComponent`, 147 wide, left of the
//! panel): what can be crafted now as a grid of 25-pixel buttons; a click crafts one.

use crate::atlas::Sprite;
use crate::draw::{DrawList, WHITE};
use crate::hud::stack;
use crate::inventory::{self, Layout};
use crate::theme::Theme;
use crate::widget::{Widgets, java};

const WIDTH: f32 = 147.0;
const CELL: f32 = 25.0;
const COLUMNS: usize = 5;
/// The grid's top-left inside the book.
const GRID: [f32; 2] = [11.0, 20.0];

const PANEL: [u8; 4] = [0xC6, 0xC6, 0xC6, 0xFF];
const EDGE: [u8; 4] = [0, 0, 0, 0xFF];
const BUTTON: [u8; 4] = [0x8B, 0x8B, 0x8B, 0xFF];
const HOVER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x80];

/// The book's rectangle and each button's, for `count` results beside `layout`.
fn layout(layout: Layout, size: [f32; 2], count: usize) -> ([f32; 4], Vec<[f32; 4]>) {
    let [ox, oy] = inventory::origin(layout, size);
    let height = layout.height();
    let book = [ox - WIDTH - 2.0, oy, ox - 2.0, oy + height];
    let rows = ((height - GRID[1] - 6.0) / CELL).floor().max(0.0) as usize;
    let cells = (0..count.min(rows * COLUMNS))
        .map(|i| {
            let (x, y) = (book[0] + GRID[0] + (i % COLUMNS) as f32 * CELL, book[1] + GRID[1] + (i / COLUMNS) as f32 * CELL);
            [x, y, x + CELL - 1.0, y + CELL - 1.0]
        })
        .collect();
    (book, cells)
}

/// The result under `mouse`, by index.
pub fn hit(screen: Layout, size: [f32; 2], count: usize, mouse: [f32; 2]) -> Option<usize> {
    layout(screen, size, count).1.iter().position(|r| (r[0]..r[2]).contains(&mouse[0]) && (r[1]..r[3]).contains(&mouse[1]))
}

/// `results`: each craftable item's icon and the stack one craft makes. Java's look draws its
/// `recipe_book.png` and `slot_craftable` buttons, Bedrock's its panel and cells, else flat greys.
pub fn draw(list: &mut DrawList, theme: &Theme, screen: Layout, results: &[(Sprite, u16)], mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    let (book, cells) = layout(screen, size, results.len());
    let java = match &theme.widgets {
        Widgets::Java(java::Kit { recipe_book: Some(page), recipe_slot: Some(slot), .. }) => Some((*page, *slot)),
        _ => None,
    };
    let bedrock = match &theme.widgets {
        Widgets::Bedrock(kit) if kit.panel.is_some() && kit.cell.is_some() => Some(kit),
        _ => None,
    };
    match (java, bedrock) {
        (Some((page, _)), _) => list.sprite_stretched(page, book, WHITE),
        (_, Some(kit)) => kit.paint(list, kit.panel, book, WHITE, 0xC6C6C6),
        _ => {
            list.fill(white, book, EDGE);
            list.fill(white, [book[0] + 1.0, book[1] + 1.0, book[2] - 1.0, book[3] - 1.0], PANEL);
        }
    }
    // Java's page has its search box where a title would go.
    if let Some(font) = theme.font.as_ref().filter(|_| java.is_none()) {
        font.draw(list, "Recipe Book", book[0] + 8.0, book[1] + 6.0, 0x404040, 1.0, false);
    }
    let hovered = hit(screen, size, results.len(), mouse);
    for (i, (rect, item)) in cells.iter().zip(results).enumerate() {
        let [x, y, ..] = *rect;
        match (java, bedrock) {
            (Some((_, slot)), _) => list.sprite(slot, x, y, WHITE),
            (_, Some(kit)) => kit.paint(list, kit.cell, *rect, WHITE, 0x8B8B8B),
            _ => list.fill(white, *rect, BUTTON),
        }
        stack(list, theme.font.as_ref(), *item, x + 4.0, y + 4.0);
        if hovered == Some(i) {
            match bedrock {
                Some(kit) => kit.paint(list, kit.highlight, [x + 4.0, y + 4.0, x + 20.0, y + 20.0], [255, 255, 255, 204], 0x62B531),
                None => list.fill(white, *rect, HOVER),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_sits_left_of_the_panel() {
        let (size, screen) = ([480.0, 270.0], Layout::Player);
        let [ox, oy] = inventory::origin(screen, size);
        let (book, cells) = layout(screen, size, 12);
        assert_eq!(book[2], ox - 2.0);
        assert_eq!(cells.len(), 12);
        assert_eq!([cells[5][0], cells[5][1]], [book[0] + 11.0, oy + 20.0 + 25.0], "a new row every five");
        assert_eq!(hit(screen, size, 12, [cells[3][0] + 1.0, cells[3][1] + 1.0]), Some(3));
    }
}
