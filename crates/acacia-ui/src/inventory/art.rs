//! What an inventory screen is drawn with: Java's container sheets (the panel with its slots
//! printed on), Bedrock's nine-slices (`dialog_background_opaque`, a `cell_image` per slot, the
//! green `highlight_slot` at 0.8), or flat greys when the look has neither.

use super::{Layout, SLOT, Slot, WIDTH};
use crate::atlas::Sprite;
use crate::draw::{DrawList, WHITE};
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

pub(super) struct Art<'a> {
    white: Sprite,
    kind: Kind<'a>,
}

enum Kind<'a> {
    Java { inventory: Sprite, top: Sprite, bottom: Sprite },
    Bedrock(&'a bedrock::Kit),
    Flat,
}

impl<'a> Art<'a> {
    pub(super) fn of(theme: &'a Theme) -> Art<'a> {
        let kind = match &theme.widgets {
            Widgets::Java(java::Kit { inventory: Some(inventory), chest_top: Some(top), chest_bottom: Some(bottom), .. }) => {
                Kind::Java { inventory: *inventory, top: *top, bottom: *bottom }
            }
            Widgets::Bedrock(kit) if kit.panel.is_some() && kit.cell.is_some() => Kind::Bedrock(kit),
            _ => Kind::Flat,
        };
        Art { white: theme.atlas.white(), kind }
    }

    /// The panel; Java's sheets bring their slots and portrait box with them.
    pub(super) fn panel(&self, list: &mut DrawList, layout: Layout, [ox, oy]: [f32; 2]) {
        let rect = [ox, oy, ox + WIDTH, oy + layout.height()];
        let portrait = [ox + PORTRAIT[0], oy + PORTRAIT[1], ox + PORTRAIT[2], oy + PORTRAIT[3]];
        match self.kind {
            Kind::Java { inventory, top, bottom } => match layout {
                Layout::Player => list.sprite_stretched(inventory, rect, WHITE),
                Layout::Rows(rows) => {
                    // ContainerScreen: the title and `rows` rows of the six-row sheet, then its player part.
                    let split = f32::from(rows) * SLOT + 17.0;
                    let texels = (split * top.width as f32 / WIDTH) as u32;
                    list.sprite_stretched(Sprite { height: texels, ..top }, [ox, oy, ox + WIDTH, oy + split], WHITE);
                    list.sprite_stretched(bottom, [ox, oy + split, ox + WIDTH, oy + split + CHEST_BOTTOM], WHITE);
                }
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

    /// One slot's backing (none for Java, whose sheet has them); an empty armour or offhand slot
    /// shows Bedrock's outline of what goes there.
    pub(super) fn slot(&self, list: &mut DrawList, slot: Slot, x: f32, y: f32, empty: bool) {
        // The player's crafting result is an ordinary 18 px slot (Java's sheet); the crafting
        // table's 26 px one is not drawn here.
        let rect = [x - 1.0, y - 1.0, x + 17.0, y + 17.0];
        match self.kind {
            Kind::Java { .. } => {}
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
            Kind::Java { .. } | Kind::Flat => list.fill(self.white, area, HOVER),
        }
    }
}

/// A raised or sunken box: `top_left` on the top and left edges, `bottom_right` on the others.
fn bevel(list: &mut DrawList, white: Sprite, [l, t, r, b]: [f32; 4], top_left: [u8; 4], bottom_right: [u8; 4], fill: [u8; 4]) {
    list.fill(white, [l, t, r, b], bottom_right);
    list.fill(white, [l, t, r - 1.0, b - 1.0], top_left);
    list.fill(white, [l + 1.0, t + 1.0, r - 1.0, b - 1.0], fill);
}
