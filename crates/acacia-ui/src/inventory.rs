//! Inventory screens: the player's own (Java's `InventoryScreen`, 176×166) and a container's rows
//! above the player's slots (`ContainerScreen`/`ChestMenu`), centred. Slot positions are Java's; the
//! panel and slots are drawn flat in vanilla's greys for either look.

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
}

impl Layout {
    pub fn height(self) -> f32 {
        match self {
            Layout::Player => 166.0,
            Layout::Rows(rows) => 114.0 + SLOT * f32::from(rows),
        }
    }

    /// Top of the player's three inventory rows (the hotbar is 58 below).
    fn player_rows(self) -> f32 {
        match self {
            Layout::Player => 84.0,
            Layout::Rows(rows) => 103.0 + SLOT * (f32::from(rows) - 4.0),
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
    /// The 2×2 grid, row by row.
    Craft(u8),
    CraftResult,
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
            out.extend((0..4u8).map(|i| (Slot::Craft(i), [98.0 + SLOT * f32::from(i % 2), 18.0 + SLOT * f32::from(i / 2)])));
            out.extend([(Slot::Offhand, [77.0, 62.0]), (Slot::CraftResult, [154.0, 28.0])]);
        }
        Layout::Rows(rows) => out.extend((0..rows * 9).map(|i| (Slot::Container(i), at(i % 9, 18.0, i / 9)))),
    }
    out
}

/// The panel's top-left for a screen `size` GUI pixels big.
pub fn origin(layout: Layout, size: [f32; 2]) -> [f32; 2] {
    [((size[0] - WIDTH) / 2.0).floor(), ((size[1] - layout.height()) / 2.0).floor()]
}

/// The slot under `mouse` (GUI pixels), if any.
pub fn hit(layout: Layout, size: [f32; 2], mouse: [f32; 2]) -> Option<Slot> {
    let [ox, oy] = origin(layout, size);
    let over = |&(_, [x, y]): &(Slot, [f32; 2])| (ox + x..ox + x + 16.0).contains(&mouse[0]) && (oy + y..oy + y + 16.0).contains(&mouse[1]);
    slots(layout).into_iter().find(over).map(|(s, _)| s)
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
}

const PANEL: [u8; 4] = [0xC6, 0xC6, 0xC6, 0xFF];
const LIGHT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const DARK: [u8; 4] = [0x55, 0x55, 0x55, 0xFF];
const SLOT_DARK: [u8; 4] = [0x37, 0x37, 0x37, 0xFF];
const SLOT_FILL: [u8; 4] = [0x8B, 0x8B, 0x8B, 0xFF];
const EDGE: [u8; 4] = [0, 0, 0, 0xFF];
/// Java's screen background gradient, flattened.
const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
const HOVER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x80];
const TITLE: u32 = 0x404040;

/// `title` names a container (the player's screen shows "Crafting").
pub fn draw(list: &mut DrawList, theme: &Theme, layout: Layout, title: &str, contents: &Contents, mouse: [f32; 2], size: [f32; 2]) {
    let white = theme.atlas.white();
    let [ox, oy] = origin(layout, size);
    let height = layout.height();
    list.fill(white, [0.0, 0.0, size[0], size[1]], SHADE);
    bevel(list, white, [ox, oy, ox + WIDTH, oy + height], LIGHT, DARK, PANEL);
    for rect in [[ox, oy, ox + WIDTH, oy + 1.0], [ox, oy + height - 1.0, ox + WIDTH, oy + height], [ox, oy, ox + 1.0, oy + height], [ox + WIDTH - 1.0, oy, ox + WIDTH, oy + height]] {
        list.fill(white, rect, EDGE);
    }
    match layout {
        Layout::Player => {
            // The player's portrait box, left of the crafting grid.
            bevel(list, white, [ox + 25.0, oy + 7.0, ox + 76.0, oy + 79.0], SLOT_DARK, LIGHT, EDGE);
            if let Some(font) = &theme.font {
                font.draw(list, "Crafting", ox + 97.0, oy + 8.0, TITLE, 1.0, false);
            }
        }
        Layout::Rows(_) => {
            if let Some(font) = &theme.font {
                font.draw(list, title, ox + 8.0, oy + 6.0, TITLE, 1.0, false);
                font.draw(list, "Inventory", ox + 8.0, oy + height - 94.0, TITLE, 1.0, false);
            }
        }
    }
    let hovered = hit(layout, size, mouse);
    for (slot, [x, y]) in slots(layout) {
        let (x, y) = (ox + x, oy + y);
        let big = if slot == Slot::CraftResult { 4.0 } else { 0.0 };
        bevel(list, white, [x - 1.0 - big, y - 1.0 - big, x + 17.0 + big, y + 17.0 + big], SLOT_DARK, LIGHT, SLOT_FILL);
        if let Some(item) = (contents.slot)(slot) {
            stack(list, theme.font.as_ref(), item, x, y);
        }
        if hovered == Some(slot) {
            list.fill(white, [x, y, x + 16.0, y + 16.0], HOVER);
        }
    }
    if let Some(item) = contents.cursor {
        stack(list, theme.font.as_ref(), item, mouse[0] - 8.0, mouse[1] - 8.0);
    }
}

/// A raised or sunken box: `top_left` on the top and left edges, `bottom_right` on the others.
fn bevel(list: &mut DrawList, white: Sprite, [l, t, r, b]: [f32; 4], top_left: [u8; 4], bottom_right: [u8; 4], fill: [u8; 4]) {
    list.fill(white, [l, t, r, b], bottom_right);
    list.fill(white, [l, t, r - 1.0, b - 1.0], top_left);
    list.fill(white, [l + 1.0, t + 1.0, r - 1.0, b - 1.0], fill);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_where_java_puts_them() {
        let p = Layout::Player;
        assert_eq!(slots(p).len(), 4 + 36 + 4 + 2);
        let size = [320.0, 240.0];
        let [ox, oy] = origin(p, size);
        assert_eq!([ox, oy], [72.0, 37.0]);
        assert_eq!(hit(p, size, [ox + 8.0 + 18.0 * 2.0 + 3.0, oy + 142.0 + 5.0]), Some(Slot::Main(2)));
        assert_eq!(hit(p, size, [ox + 8.0 + 1.0, oy + 84.0 + 1.0]), Some(Slot::Main(9)));
        assert_eq!(hit(p, size, [ox + 8.0 + 18.0 * 8.0 + 1.0, oy + 84.0 + 36.0 + 1.0]), Some(Slot::Main(35)));
        assert_eq!(hit(p, size, [ox + 9.0, oy + 9.0 + 18.0 * 3.0]), Some(Slot::Armor(3)));
        assert_eq!(hit(p, size, [ox + 7.0, oy + 142.0]), None, "the gap between slots");
        assert!(!inside(p, size, [10.0, 10.0]));
    }

    #[test]
    fn a_chest_sits_above_the_player_rows() {
        let chest = Layout::Rows(3);
        assert_eq!(chest.height(), 168.0);
        let all = slots(chest);
        assert_eq!(all.len(), 27 + 36);
        let at = |s| all.iter().find(|(slot, _)| *slot == s).unwrap().1;
        assert_eq!(at(Slot::Container(0)), [8.0, 18.0]);
        assert_eq!(at(Slot::Container(26)), [8.0 + 18.0 * 8.0, 18.0 + 36.0]);
        assert_eq!(at(Slot::Main(9)), [8.0, 85.0]);
        assert_eq!(at(Slot::Main(0)), [8.0, 143.0]);
        assert_eq!(Layout::Rows(6).height(), 222.0);
    }
}
