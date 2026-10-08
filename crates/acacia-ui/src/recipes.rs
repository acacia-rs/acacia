//! The recipe book beside an inventory screen (Java's `RecipeBookComponent`, 147 wide, left of the
//! panel): what can be crafted now as a grid of 25-pixel buttons; a click crafts one.

use crate::atlas::Sprite;
use crate::draw::DrawList;
use crate::hud::stack;
use crate::inventory::{self, Layout};
use crate::theme::Theme;

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

/// `results`: each craftable item's icon and the stack one craft makes.
pub fn draw(list: &mut DrawList, theme: &Theme, screen: Layout, results: &[(Sprite, u16)], mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    let (book, cells) = layout(screen, size, results.len());
    list.fill(white, book, EDGE);
    list.fill(white, [book[0] + 1.0, book[1] + 1.0, book[2] - 1.0, book[3] - 1.0], PANEL);
    if let Some(font) = &theme.font {
        font.draw(list, "Recipe Book", book[0] + 8.0, book[1] + 6.0, 0x404040, 1.0, false);
    }
    let hovered = hit(screen, size, results.len(), mouse);
    for (i, (rect, item)) in cells.iter().zip(results).enumerate() {
        list.fill(white, *rect, BUTTON);
        stack(list, theme.font.as_ref(), *item, rect[0] + 4.0, rect[1] + 4.0);
        if hovered == Some(i) {
            list.fill(white, *rect, HOVER);
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
