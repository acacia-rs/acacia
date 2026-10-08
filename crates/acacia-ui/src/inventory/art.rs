//! What an inventory screen is drawn with: Java's container sheets (the panel with its slots
//! printed on), Bedrock's nine-slices (`dialog_background_opaque`, a `cell_image` per slot, the
//! green `highlight_slot` at 0.8), or flat greys when the look lacks the screen's art.

use super::{Layout, Progress, SLOT, Slot, Station, WIDTH};
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
                };
                sheets.map_or(Kind::Flat, |(sheet, bottom)| Kind::Java(kit, sheet, bottom))
            }
            Widgets::Bedrock(kit) if kit.panel.is_some() && kit.cell.is_some() => Kind::Bedrock(kit),
            Widgets::Bedrock(_) => Kind::Flat,
        };
        Art { white: theme.atlas.white(), kind }
    }

    /// The panel; Java's sheets bring their slots and portrait box with them.
    pub(super) fn panel(&self, list: &mut DrawList, layout: Layout, [ox, oy]: [f32; 2]) {
        let rect = [ox, oy, ox + WIDTH, oy + layout.height()];
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
                _ => list.sprite_stretched(sheet, rect, WHITE),
            },
            Kind::Bedrock(kit) => {
                kit.paint(list, kit.panel, rect, WHITE, 0xC6C6C6);
                if layout == Layout::Player {
                    list.fill(self.white, portrait, EDGE);
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
        // The player's crafting result is an ordinary 18 px slot (Java's sheet); the crafting
        // table's 26 px one is not drawn here.
        let rect = [x - 1.0, y - 1.0, x + 17.0, y + 17.0];
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
