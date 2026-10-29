//! Drawing an inventory screen: the panel, titles, tabs, the pick list and the slots.

use super::*;
use crate::widget::{State, Widget};

/// Java's screen background gradient, flattened.
const SHADE: [u8; 4] = [0x10, 0x10, 0x10, 0xC8];
const TITLE: u32 = 0x404040;
/// What the creative tabs' 48-pixel buttons say.
const TAB_BUTTONS: [&str; 6] = ["Build", "Nature", "Tools", "Items", "Self", "Find"];

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
                if !(bench.name_hides_title() && contents.name.is_some()) {
                    font.draw(list, bench.title(), ox + x, oy + y, TITLE, 1.0, false);
                }
                // BeaconScreen: two headings and no "Inventory"; light text on Java's dark boxes.
                let (colour, shadow) = if matches!(theme.widgets, crate::widget::Widgets::Java(_)) { (0xE0E0E0, true) } else { (TITLE, false) };
                for (heading, [middle, y]) in super::bench::BEACON_HEADINGS.into_iter().filter(|_| bench == Bench::Beacon) {
                    font.draw(list, heading, ox + middle - (font.width(heading) / 2.0).floor(), oy + y, colour, 1.0, shadow);
                }
                if bench == Bench::Beacon { 0.0 } else { font.draw(list, "Inventory", ox + 8.0, oy + height - 94.0, TITLE, 1.0, false) }
            }
            // MerchantScreen: the trader's name over the right part, "Trades" over the offers.
            Layout::Creative => font.draw(list, title, ox + 8.0, oy + 6.0, TITLE, 1.0, false),
            Layout::Trade => {
                let left = layout.player_x();
                font.draw(list, "Trades", ox + 5.0 + ((88.0 - font.width("Trades")) / 2.0).floor(), oy + 6.0, TITLE, 1.0, false);
                font.draw(list, title, ox + left + ((162.0 - font.width(title)) / 2.0).floor(), oy + 6.0, TITLE, 1.0, false);
                font.draw(list, "Inventory", ox + left, oy + height - 94.0, TITLE, 1.0, false)
            }
        };
    }
    if let (Layout::Bench(bench), Some((before, after))) = (layout, contents.name)
        && let Some([x, y]) = bench.name_box()
    {
        art.name_box(list, [ox + x, oy + y], !(before.is_empty() && after.is_empty()));
        if let Some(font) = &theme.font {
            let (start, caret) = (ox + x + 3.0, ox + x + 3.0 + font.width(before));
            font.draw(list, before, start, oy + y + 4.0, 0xFFFFFF, 1.0, true);
            font.draw(list, "_", caret, oy + y + 5.0, 0xFFFFFF, 1.0, true);
            font.draw(list, after, caret, oy + y + 4.0, 0xFFFFFF, 1.0, true);
        }
    }
    if let (Layout::Trade, Some(share)) = (layout, contents.experience) {
        // MerchantScreen's experience bar: 102×5 under the trader's name.
        let (white, [x, y]) = (theme.atlas.white(), [ox + 136.0, oy + 16.0]);
        list.fill(white, [x, y, x + 102.0, y + 5.0], [0x2B, 0x2B, 0x2B, 0xFF]);
        list.fill(white, [x + 1.0, y + 1.0, x + 1.0 + 100.0 * share.clamp(0.0, 1.0), y + 4.0], [0x80, 0xFF, 0x20, 0xFF]);
    }
    if let (Some((note, payable)), Some(font)) = (contents.note, &theme.font) {
        // AnvilScreen's cost line: right-aligned on a dark strip, red when it cannot be paid.
        let (right, x) = (ox + layout.width() - 8.0, ox + layout.width() - 10.0 - font.width(note));
        list.fill(theme.atlas.white(), [x - 2.0, oy + 67.0, right, oy + 79.0], [0, 0, 0, 0x4F]);
        font.draw(list, note, x, oy + 69.0, if payable { 0x80FF20 } else { 0xFF6060 }, 1.0, true);
    }
    if let (Some(scrolled), Some(([x, y], [w, h], travel))) = (contents.scrolled, layout.scroller()) {
        let (left, top) = (ox + x, oy + y + travel * scrolled.clamp(0.0, 1.0));
        list.fill(theme.atlas.white(), [left, top, left + w, top + h], [0x55, 0x55, 0x55, 0xFF]);
        list.fill(theme.atlas.white(), [left + 1.0, top + 1.0, left + w - 1.0, top + h - 1.0], [0xC6, 0xC6, 0xC6, 0xFF]);
    }
    if let Some((open, scrolled)) = contents.creative {
        let skin = theme.widgets.skin();
        for (i, rect) in tab_rects(layout, size).into_iter().enumerate() {
            let state = State { hover: contains(rect, mouse), pressed: i == open, ..State::default() };
            skin.draw(list, theme.font.as_ref(), &Widget::Button { text: TAB_BUTTONS[i].into(), image: None }, rect, state);
        }
        // CreativeModeInventoryScreen's scroller: 12×15 in a 112-pixel track.
        let top = oy + 18.0 + (112.0 - 15.0) * scrolled.clamp(0.0, 1.0);
        if layout == Layout::Creative {
            list.fill(theme.atlas.white(), [ox + 175.0, top, ox + 187.0, top + 15.0], [0x55, 0x55, 0x55, 0xFF]);
            list.fill(theme.atlas.white(), [ox + 176.0, top + 1.0, ox + 186.0, top + 14.0], [0xC6, 0xC6, 0xC6, 0xFF]);
        }
    }
    let over = hit_pick(layout, size, contents.picks.len(), mouse);
    for (i, (rect, item)) in pick_rects(layout, size, contents.picks.len()).into_iter().zip(contents.picks).enumerate() {
        art.pick(list, rect, contents.picked.contains(&Some(i)), over == Some(i) && !item.closed, item.closed);
        let (inset, text) = (((rect[3] - rect[1] - 16.0) / 2.0).floor(), layout.picks().map_or(5.0, |p| p.text));
        for &(icon, count, x) in &item.icons {
            // Pick lists carry plain sprites: their stacks do not glint.
            stack(list, theme.font.as_ref(), Item { icon, count, glint: false }, rect[0] + x, rect[1] + inset);
        }
        // An option with galactic words has two lines: the words, then what they stand for.
        let runes = theme.galactic.as_ref().filter(|_| !item.script.is_empty());
        if let Some(galactic) = runes {
            let room = rect[2] - rect[0] - text - 2.0;
            let fits = (0..=item.script.len()).rev().filter(|&end| item.script.is_char_boundary(end)).find(|&end| galactic.width(&item.script[..end]) <= room);
            galactic.draw(list, &item.script[..fits.unwrap_or(0)], rect[0] + text, rect[1] + 1.0, art.pick_text(), 1.0, false);
        }
        if let Some(font) = &theme.font {
            let y = if runes.is_some() { rect[1] + 10.0 } else { rect[1] + inset + 4.0 };
            font.draw(list, &item.label, rect[0] + text, y, art.pick_text(), 1.0, false);
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
    if let (Layout::Bench(Bench::Loom), Some(made)) = (layout, (contents.slot)(Slot::Result)) {
        // LoomScreen shows the banner the pattern makes, large, above the result.
        list.icon(made.icon, [ox + 135.0, oy + 12.0, ox + 167.0, oy + 44.0], false);
    }
    if let Some(item) = contents.cursor {
        stack(list, theme.font.as_ref(), item, mouse[0] - 8.0, mouse[1] - 8.0);
    }
}
