//! The sheets of the maps held in first person, rebuilt when the bot's picture changes.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use acacia_bot::proto::types::MapDecorationType as Kind;
use acacia_bot::state::{MAP_SIZE, MapImage};
use acacia_render::entity::Skin;
use acacia_render::item::map::{self, Marker};

/// Both hands can hold a map.
const KEPT: usize = 2;

thread_local! {
    /// The window thread's: the last pictures drawn and their sheets, the newest first.
    static SHEETS: RefCell<Vec<(Arc<MapImage>, Arc<Skin>)>> = const { RefCell::new(Vec::new()) };
}

pub fn sheet(root: &Path, picture: &Arc<MapImage>) -> Option<Arc<Skin>> {
    SHEETS.with_borrow_mut(|sheets| {
        if let Some((_, sheet)) = sheets.iter().find(|(from, _)| Arc::ptr_eq(from, picture)) {
            return Some(sheet.clone());
        }
        let sheet = Arc::new(map::skin(root, &picture.rgba, &markers(picture))?);
        sheets.truncate(KEPT - 1);
        sheets.insert(0, (picture.clone(), sheet.clone()));
        Some(sheet)
    })
}

/// The picture's markers as the renderer draws them. A marker with no turn points south (down
/// the picture), as a player with no yaw looks.
pub fn markers(picture: &MapImage) -> Vec<Marker> {
    let middle = MAP_SIZE as f32 / 2.0;
    let marker = |m: &acacia_bot::state::MapMarker| Marker {
        at: m.offset.map(|half_pixels| middle + f32::from(half_pixels) / 2.0),
        turn: 180.0 + f32::from(m.rotation) * 22.5,
        colour: match m.kind {
            Kind::MarkerGreen => [0x40, 0xC0, 0x40],
            Kind::MarkerRed | Kind::TriangleRed => [0xD0, 0x30, 0x30],
            Kind::MarkerBlue => [0x40, 0x60, 0xE0],
            _ => [0xFF; 3],
        },
    };
    picture.markers.iter().map(marker).collect()
}
