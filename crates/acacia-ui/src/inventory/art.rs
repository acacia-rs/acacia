//! What an inventory screen is drawn with: Java's container sheets (the panel with its slots
//! printed on), Bedrock's nine-slices (`dialog_background_opaque`, a `cell_image` per slot, the
//! green `highlight_slot` at 0.8), or flat greys when the look lacks the screen's art.

use super::{Bench, Layout, Progress, SLOT, Slot, Station, WIDTH};
use crate::atlas::Sprite;
use crate::draw::{DrawList, WHITE};
use crate::nine::Nine;
use crate::theme::Theme;
use crate::widget::{Widgets, bedrock, java};

const PANEL: [u8; 4] = [0xC6, 0xC6, 0xC6, 0xFF];
const LIGHT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const DARK: [u8; 4] = [0x55, 0x55, 0x55, 0xFF];
const SLOT_DARK: [u8; 4] = [0x37, 0x37, 0x37, 0xFF];
const SLOT_FILL: [u8; 4] = [0x8B, 0x8B, 0x8B, 0xFF];
const EDGE: [u8; 4] = [0, 0, 0, 0xFF];
/// Java's hovered slot, `0x80FFFFFF`.
const HOVER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0x80];
/// Bedrock's `highlight_slot` alpha.
const HIGHLIGHT: [u8; 4] = [0xFF, 0xFF, 0xFF, 204];
/// The player portrait's box, relative to the panel.
const PORTRAIT: [f32; 4] = [25.0, 7.0, 76.0, 79.0];
/// Height of the player's part of Java's chest sheet.
const CHEST_BOTTOM: f32 = 96.0;
/// Where Java draws a furnace's flame and arrow, and a brewing stand's fuel bar and arrow.
const FLAME: [f32; 2] = [56.0, 36.0];
const ARROW: [f32; 2] = [79.0, 34.0];
const BREW_FUEL: [f32; 2] = [60.0, 44.0];
const BREW_ARROW: [f32; 2] = [97.0, 16.0];

pub(super) struct Art<'a> {
    white: Sprite,
    kind: Kind<'a>,
    /// The open bench: Bedrock's panel gets its arrow drawn on.
    bench: Option<Bench>,
    /// The trade screen: its offers are the look's buttons.
    trade: bool,
    /// The creative inventory: its picks are slots.
    creative: bool,
}

enum Kind<'a> {
    /// The sheet(s) for the layout: the whole panel, or a chest's rows and its player part.
    Java(&'a java::Kit, Sprite, Option<Sprite>),
    Bedrock(&'a bedrock::Kit),
    Flat,
}

impl<'a> Art<'a> {
    pub(super) fn of(theme: &'a Theme, layout: Layout) -> Art<'a> {
        let kind = match &theme.widgets {
            Widgets::Java(kit) => {
                let sheets = match layout {
                    Layout::Player => kit.inventory.map(|s| (s, None)),
                    Layout::Rows(_) => kit.chest_top.zip(kit.chest_bottom).map(|(top, bottom)| (top, Some(bottom))),
                    Layout::Station(station) => kit.stations.get(station.sheet()).map(|s| (*s, None)),
                    Layout::Bench(bench) => kit.stations.get(bench.sheet()).map(|s| (*s, None)),
                    Layout::Trade => kit.trade.map(|s| (s, None)),
                    Layout::Creative => kit.creative.map(|s| (s, None)),
                };
                sheets.map_or(Kind::Flat, |(sheet, bottom)| Kind::Java(kit, sheet, bottom))
            }
            Widgets::Bedrock(kit) if kit.panel.is_some() && kit.cell.is_some() => Kind::Bedrock(kit),
            Widgets::Bedrock(_) => Kind::Flat,
        };
        let bench = if let Layout::Bench(bench) = layout { Some(bench) } else { None };
        Art { white: theme.atlas.white(), kind, bench, trade: layout == Layout::Trade, creative: layout == Layout::Creative }
    }

    /// The panel; Java's sheets bring their slots and portrait box with them.
    pub(super) fn panel(&self, list: &mut DrawList, layout: Layout, [ox, oy]: [f32; 2]) {
        let rect = [ox, oy, ox + layout.width(), oy + layout.height()];
        let portrait = [ox + PORTRAIT[0], oy + PORTRAIT[1], ox + PORTRAIT[2], oy + PORTRAIT[3]];
        match self.kind {
            Kind::Java(_, sheet, bottom) => match (layout, bottom) {
                // ContainerScreen: the title and the rows of the six-row sheet, then its player part.
                (Layout::Rows(rows), Some(bottom)) => {
                    let split = f32::from(rows) * SLOT + 17.0;
                    let texels = (split * sheet.width as f32 / WIDTH) as u32;
                    list.sprite_stretched(Sprite { height: texels, ..sheet }, [ox, oy, ox + WIDTH, oy + split], WHITE);
                    list.sprite_stretched(bottom, [ox, oy + split, ox + WIDTH, oy + split + CHEST_BOTTOM], WHITE);
                }
                _ => {
                    list.sprite_stretched(sheet, rect, WHITE);
                }
            },
            Kind::Bedrock(kit) => {
                kit.paint(list, kit.panel, rect, WHITE, 0xC6C6C6);
                if layout == Layout::Player {
                    list.fill(self.white, portrait, EDGE);
                }
                if let (Some([x, y]), Some(Nine { sprite, .. })) = (self.bench.and_then(Bench::arrow), kit.arrow[0]) {
                    list.sprite(sprite, ox + x, oy + y, WHITE);
                }
            }
            Kind::Flat => {
                bevel(list, self.white, rect, LIGHT, DARK, PANEL);
                let [l, t, r, b] = rect;
                for edge in [[l, t, r, t + 1.0], [l, b - 1.0, r, b], [l, t, l + 1.0, b], [r - 1.0, t, r, b]] {
                    list.fill(self.white, edge, EDGE);
                }
                if layout == Layout::Player {
                    bevel(list, self.white, portrait, SLOT_DARK, LIGHT, EDGE);
                }
            }
        }
    }

    /// A furnace's flame (burning down) and arrow (filling rightwards); a brewing stand's fuel bar
    /// and its arrow (filling downwards). Java's sheets have the empty shapes printed on.
    pub(super) fn progress(&self, list: &mut DrawList, station: Station, progress: Progress, [ox, oy]: [f32; 2]) {
        let (work, fuel) = (progress.work.clamp(0.0, 1.0), progress.fuel.clamp(0.0, 1.0));
        let at = |p: [f32; 2]| [ox + p[0], oy + p[1]];
        let brewing = station == Station::Brewing;
        if !brewing && !station.is_furnace() {
            return;
        }
        let (fuel_at, work_at) = if brewing { (at(BREW_FUEL), at(BREW_ARROW)) } else { (at(FLAME), at(ARROW)) };
        // Which way each fills: the flame keeps its bottom, the brewing arrow its top, the rest their left.
        let (fuel_grows, work_grows) = if brewing { (Grow::Right, Grow::Down) } else { (Grow::Up, Grow::Right) };
        match self.kind {
            Kind::Java(kit, ..) => {
                let (fuel_name, work_name) = if brewing { ("fuel_length", "brew_progress") } else { ("lit_progress", "burn_progress") };
                let sprite = |name: &str| kit.stations.get(&format!("{}/{name}", station.sheet())).copied();
                for (sprite, [x, y], share, grow) in [(sprite(fuel_name), fuel_at, fuel, fuel_grows), (sprite(work_name), work_at, work, work_grows)] {
                    if let Some(sprite) = sprite {
                        part(list, sprite, x, y, share, grow);
                    }
                }
            }
            Kind::Bedrock(kit) => {
                let (fuel_art, work_art) = if brewing { (kit.brew_fuel, kit.brew_arrow) } else { (kit.flame, kit.arrow) };
                for ([empty, full], [x, y], share, grow) in [(fuel_art, fuel_at, fuel, fuel_grows), (work_art, work_at, work, work_grows)] {
                    if let Some(Nine { sprite, .. }) = empty {
                        list.sprite(sprite, x, y, WHITE);
                    }
                    if let Some(Nine { sprite, .. }) = full {
                        part(list, sprite, x, y, share, grow);
                    }
                }
            }
            Kind::Flat => {}
        }
    }

    /// One slot's backing (none for Java, whose sheet has them); an empty armour or offhand slot
    /// shows Bedrock's outline of what goes there.
    pub(super) fn slot(&self, list: &mut DrawList, slot: Slot, x: f32, y: f32, empty: bool) {
        // The player's crafting result is an ordinary 18 px slot (Java's sheet), a bench's is 26.
        let pad = if slot == Slot::Result && self.bench.is_some_and(Bench::big_result) { 5.0 } else { 1.0 };
        let rect = [x - pad, y - pad, x + 16.0 + pad, y + 16.0 + pad];
        match self.kind {
            Kind::Java(..) => {}
            Kind::Bedrock(kit) => {
                kit.paint(list, kit.cell, rect, WHITE, 0x8B8B8B);
                let outline = match slot {
                    Slot::Armor(i) => kit.empty_slots.get(usize::from(i)).copied().flatten(),
                    Slot::Offhand => kit.empty_slots[4],
                    _ => None,
                };
                if let Some(outline) = outline.filter(|_| empty) {
                    list.nine(outline, [x, y, x + 16.0, y + 16.0], WHITE);
                }
            }
            Kind::Flat => bevel(list, self.white, rect, SLOT_DARK, LIGHT, SLOT_FILL),
        }
    }

    /// A pick-list button: Java's stonecutter or enchanting sprites, Bedrock's cell with its highlight.
    pub(super) fn pick(&self, list: &mut DrawList, rect: [f32; 4], picked: bool, hover: bool) {
        let flat = if picked { SLOT_FILL } else { PANEL };
        if self.creative {
            let [x, y] = [rect[0] + 1.0, rect[1] + 1.0];
            self.slot(list, Slot::Container(0), x, y, false);
            if hover {
                self.hover(list, x, y);
            }
            return;
        }
        match self.kind {
            Kind::Java(kit, ..) if self.trade => match kit.button_nine(hover) {
                Some(button) => list.nine(button, rect, WHITE),
                None => bevel(list, self.white, rect, LIGHT, DARK, flat),
            },
            Kind::Java(kit, ..) => {
                let name = match (self.bench, picked, hover) {
                    (Some(Bench::Enchanting), _, true) => "enchanting_table/enchantment_slot_highlighted",
                    (Some(Bench::Enchanting), ..) => "enchanting_table/enchantment_slot",
                    (_, true, _) => "stonecutter/recipe_selected",
                    (_, _, true) => "stonecutter/recipe_highlighted",
                    _ => "stonecutter/recipe",
                };
                match kit.stations.get(name) {
                    Some(sprite) => list.sprite(*sprite, rect[0], rect[1], WHITE),
                    None => bevel(list, self.white, rect, LIGHT, DARK, flat),
                }
            }
            Kind::Bedrock(kit) => {
                kit.paint(list, kit.cell, rect, WHITE, 0x8B8B8B);
                if picked || hover {
                    kit.paint(list, kit.highlight, [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 1.0, rect[3] - 1.0], HIGHLIGHT, 0x62B531);
                }
            }
            Kind::Flat => bevel(list, self.white, rect, LIGHT, DARK, flat),
        }
    }

    /// A bench's name box (110×16): Java's anvil sprites, a dark box otherwise.
    pub(super) fn name_box(&self, list: &mut DrawList, [x, y]: [f32; 2], filled: bool) {
        let rect = [x, y, x + 110.0, y + 16.0];
        match self.kind {
            Kind::Java(kit, ..) => {
                let name = if filled { "anvil/text_field" } else { "anvil/text_field_disabled" };
                match kit.stations.get(name) {
                    Some(field) => list.sprite(*field, x, y, WHITE),
                    None => bevel(list, self.white, rect, SLOT_DARK, LIGHT, EDGE),
                }
            }
            _ => bevel(list, self.white, rect, SLOT_DARK, LIGHT, EDGE),
        }
    }

    /// Text on a pick button: Java's enchanting option brown, dark grey on Bedrock's cells.
    pub(super) fn pick_text(&self) -> u32 {
        match self.kind {
            Kind::Java(..) if self.trade => 0xFFFFFF,
            Kind::Java(..) => 0x685E4A,
            _ => 0x404040,
        }
    }

    /// Bedrock's green highlight lies under the item, Java's white veil over it.
    pub(super) fn hover_is_under(&self) -> bool {
        matches!(self.kind, Kind::Bedrock(_))
    }

    pub(super) fn hover(&self, list: &mut DrawList, x: f32, y: f32) {
        let area = [x, y, x + 16.0, y + 16.0];
        match self.kind {
            Kind::Bedrock(kit) => kit.paint(list, kit.highlight, area, HIGHLIGHT, 0x62B531),
            Kind::Java(..) | Kind::Flat => list.fill(self.white, area, HOVER),
        }
    }
}

#[derive(Clone, Copy)]
enum Grow {
    Right,
    Up,
    Down,
}

/// The `share` of `sprite` (0 to 1) that has filled in, placed where the whole would lie at (`x`, `y`).
fn part(list: &mut DrawList, sprite: Sprite, x: f32, y: f32, share: f32, grow: Grow) {
    let (w, h) = (sprite.width as f32, sprite.height as f32);
    if share <= 0.0 {
        return;
    }
    match grow {
        Grow::Right => list.sprite_part(sprite, x, y, [0.0, 0.0, (w * share).ceil(), h], WHITE),
        Grow::Down => list.sprite_part(sprite, x, y, [0.0, 0.0, w, (h * share).ceil()], WHITE),
        Grow::Up => {
            let cut = h - (h * share).ceil();
            list.sprite_part(sprite, x, y + cut, [0.0, cut, w, h], WHITE);
        }
    }
}

/// A raised or sunken box: `top_left` on the top and left edges, `bottom_right` on the others.
fn bevel(list: &mut DrawList, white: Sprite, [l, t, r, b]: [f32; 4], top_left: [u8; 4], bottom_right: [u8; 4], fill: [u8; 4]) {
    list.fill(white, [l, t, r, b], bottom_right);
    list.fill(white, [l, t, r - 1.0, b - 1.0], top_left);
    list.fill(white, [l + 1.0, t + 1.0, r - 1.0, b - 1.0], fill);
}
