//! Inventory screens: the player's own (Java's `InventoryScreen`, 176×166) and a container's rows
//! above the player's slots (`ContainerScreen`/`ChestMenu`), centred. Slot positions are Java's
//! (Bedrock's classic screen has the same 176×166 root); the panel is each game's art (`art.rs`).

mod art;
mod bench;
mod station;

pub use bench::{Bench, Picks};
pub use station::{Progress, Station};

use crate::atlas::Sprite;
use crate::draw::DrawList;
use crate::hud::stack;
use crate::theme::Theme;

pub const WIDTH: f32 = 176.0;
const SLOT: f32 = 18.0;

/// Which screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Armour, offhand, the 2×2 crafting grid and the player's slots.
    Player,
    /// A container of this many rows of nine (chest 3, large chest 6, barrel 3, shulker box 3).
    Rows(u8),
    /// A furnace, hopper, dispenser or brewing stand (`station.rs`).
    Station(Station),
    /// A crafting table, anvil, enchanting table and the like (`bench.rs`).
    Bench(Bench),
}

impl Layout {
    pub fn height(self) -> f32 {
        match self {
            Layout::Player => 166.0,
            Layout::Rows(rows) => 114.0 + SLOT * f32::from(rows),
            Layout::Station(station) => station.height(),
            Layout::Bench(_) => 166.0,
        }
    }

    /// Whether the recipe book stands beside the screen.
    pub fn has_book(self) -> bool {
        matches!(self, Layout::Player | Layout::Bench(Bench::Crafting))
    }

    /// Top of the player's three inventory rows (the hotbar is 58 below).
    fn player_rows(self) -> f32 {
        match self {
            Layout::Player => 84.0,
            Layout::Rows(rows) => 103.0 + SLOT * (f32::from(rows) - 4.0),
            Layout::Station(station) => station.player_rows(),
            Layout::Bench(_) => 84.0,
        }
    }
}

/// A slot of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// 0-8 the hotbar, 9-35 the rows above it.
    Main(u8),
    /// Helmet, chestplate, leggings, boots.
    Armor(u8),
    Offhand,
    /// A workstation slot of the player's UI window, by Bedrock's offset (the crafting grids, an
    /// anvil's inputs).
    Ui(u8),
    /// What the workstation's slots make.
    Result,
    /// The open container's slots, row by row.
    Container(u8),
}

/// Every slot of `layout` with the top-left of its 16×16 item area, relative to the panel.
pub fn slots(layout: Layout) -> Vec<(Slot, [f32; 2])> {
    let at = |col: u8, top: f32, row: u8| [8.0 + SLOT * f32::from(col), top + SLOT * f32::from(row)];
    let top = layout.player_rows();
    let mut out: Vec<(Slot, [f32; 2])> = (0..27u8).map(|i| (Slot::Main(9 + i), at(i % 9, top, i / 9))).collect();
    out.extend((0..9u8).map(|i| (Slot::Main(i), at(i, top + 58.0, 0))));
    match layout {
        Layout::Player => {
            out.extend((0..4u8).map(|i| (Slot::Armor(i), [8.0, 8.0 + SLOT * f32::from(i)])));
            out.extend((0..4u8).map(|i| (Slot::Ui(bench::GRID_2X2 + i), [98.0 + SLOT * f32::from(i % 2), 18.0 + SLOT * f32::from(i / 2)])));
            out.extend([(Slot::Offhand, [77.0, 62.0]), (Slot::Result, [154.0, 28.0])]);
        }
        Layout::Rows(rows) => out.extend((0..rows * 9).map(|i| (Slot::Container(i), at(i % 9, 18.0, i / 9)))),
        Layout::Station(station) => out.extend(station.slots().into_iter().enumerate().map(|(i, at)| (Slot::Container(i as u8), at))),
        Layout::Bench(bench) => out.extend(bench.slots()),
    }
    out
}

/// The panel's top-left for a screen `size` GUI pixels big.
/// A screen with the recipe book moves right to make room for it, as Java's does when the book
/// is open and the screen is at least 379 wide.
pub fn origin(layout: Layout, size: [f32; 2]) -> [f32; 2] {
    let book = if layout.has_book() && size[0] >= BOOK_ROOM { BOOK_SHIFT } else { 0.0 };
    [((size[0] - WIDTH) / 2.0).floor() + book, ((size[1] - layout.height()) / 2.0).floor()]
}

const BOOK_ROOM: f32 = 379.0;
const BOOK_SHIFT: f32 = 77.0;

/// The slot under `mouse` (GUI pixels), if any.
pub fn hit(layout: Layout, size: [f32; 2], mouse: [f32; 2]) -> Option<Slot> {
    let [ox, oy] = origin(layout, size);
    let over = |&(_, [x, y]): &(Slot, [f32; 2])| (ox + x..ox + x + 16.0).contains(&mouse[0]) && (oy + y..oy + y + 16.0).contains(&mouse[1]);
    slots(layout).into_iter().find(over).map(|(s, _)| s)
}

/// Each of a bench's first `count` pick buttons, as far as its list shows them.
fn pick_rects(layout: Layout, size: [f32; 2], count: usize) -> Vec<[f32; 4]> {
    let Layout::Bench(bench) = layout else { return Vec::new() };
    let Some(Picks { at, columns, rows, cell }) = bench.picks() else { return Vec::new() };
    let [ox, oy] = origin(layout, size);
    (0..count.min(columns * rows))
        .map(|i| {
            let (x, y) = (ox + at[0] + (i % columns) as f32 * cell[0], oy + at[1] + (i / columns) as f32 * cell[1]);
            [x, y, x + cell[0], y + cell[1]]
        })
        .collect()
}

/// The pick button under `mouse`, of `count`.
pub fn hit_pick(layout: Layout, size: [f32; 2], count: usize, mouse: [f32; 2]) -> Option<usize> {
    pick_rects(layout, size, count).iter().position(|r| (r[0]..r[2]).contains(&mouse[0]) && (r[1]..r[3]).contains(&mouse[1]))
}

/// Whether `mouse` is over the panel (a click outside it drops the held stack).
pub fn inside(layout: Layout, size: [f32; 2], mouse: [f32; 2]) -> bool {
    let [ox, oy] = origin(layout, size);
    (ox..ox + WIDTH).contains(&mouse[0]) && (oy..oy + layout.height()).contains(&mouse[1])
}

/// What is in each slot, and the stack the mouse carries.
pub struct Contents<'a> {
    pub slot: &'a dyn Fn(Slot) -> Option<(Sprite, u16)>,
    pub cursor: Option<(Sprite, u16)>,
    /// A station's arrow and flame.
    pub progress: Progress,
    /// What a bench with a pick list offers, and the one chosen.
    pub picks: &'a [Pick],
    pub picked: Option<usize>,
}

/// One entry of a bench's pick list: a result's icon (the stonecutter's) or a line of text (an
/// enchanting option).
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub icon: Option<Sprite>,
    pub label: String,
}

/// Java's screen background gradient, flattened.
const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
const TITLE: u32 = 0x404040;

/// `title` names a container (the player's screen shows "Crafting").
pub fn draw(list: &mut DrawList, theme: &Theme, layout: Layout, title: &str, contents: &Contents, mouse: [f32; 2], size: [f32; 2]) {
    let [ox, oy] = origin(layout, size);
    let height = layout.height();
    list.fill(theme.atlas.white(), [0.0, 0.0, size[0], size[1]], SHADE);
    let art = art::Art::of(theme, layout);
    art.panel(list, layout, [ox, oy]);
    if let Layout::Station(station) = layout {
        art.progress(list, station, contents.progress, [ox, oy]);
    }
    if let Some(font) = &theme.font {
        match layout {
            Layout::Player => font.draw(list, "Crafting", ox + 97.0, oy + 8.0, TITLE, 1.0, false),
            Layout::Rows(_) => {
                font.draw(list, title, ox + 8.0, oy + 6.0, TITLE, 1.0, false);
                font.draw(list, "Inventory", ox + 8.0, oy + height - 94.0, TITLE, 1.0, false)
            }
            // Java centres a furnace's title; the others' start at 8 like a chest's.
            Layout::Station(station) => {
                let x = if station.is_furnace() || station == Station::Brewing { ((WIDTH - font.width(title)) / 2.0).floor() } else { 8.0 };
                font.draw(list, title, ox + x, oy + 6.0, TITLE, 1.0, false);
                font.draw(list, "Inventory", ox + 8.0, oy + height - 94.0, TITLE, 1.0, false)
            }
            Layout::Bench(bench) => {
                let [x, y] = bench.title_at();
                font.draw(list, bench.title(), ox + x, oy + y, TITLE, 1.0, false);
                font.draw(list, "Inventory", ox + 8.0, oy + height - 94.0, TITLE, 1.0, false)
            }
        };
    }
    let over = hit_pick(layout, size, contents.picks.len(), mouse);
    for (i, (rect, item)) in pick_rects(layout, size, contents.picks.len()).into_iter().zip(contents.picks).enumerate() {
        art.pick(list, rect, contents.picked == Some(i), over == Some(i));
        if let Some(icon) = item.icon {
            stack(list, theme.font.as_ref(), (icon, 1), rect[0], rect[1] + 1.0);
        }
        if let Some(font) = &theme.font {
            font.draw(list, &item.label, rect[0] + 5.0, rect[1] + 5.0, art.pick_text(), 1.0, false);
        }
    }
    let hovered = hit(layout, size, mouse);
    for (slot, [x, y]) in slots(layout) {
        let (x, y) = (ox + x, oy + y);
        let item = (contents.slot)(slot);
        art.slot(list, slot, x, y, item.is_none());
        let hover = hovered == Some(slot);
        if hover && art.hover_is_under() {
            art.hover(list, x, y);
        }
        if let Some(item) = item {
            stack(list, theme.font.as_ref(), item, x, y);
        }
        if hover && !art.hover_is_under() {
            art.hover(list, x, y);
        }
    }
    if let Some(item) = contents.cursor {
        stack(list, theme.font.as_ref(), item, mouse[0] - 8.0, mouse[1] - 8.0);
    }
}

#[cfg(test)]
mod tests;
