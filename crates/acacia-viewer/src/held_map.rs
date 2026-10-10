//! The sheets of the maps held in first person, rebuilt when the bot's picture changes.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use acacia_bot::state::MapImage;
use acacia_render::entity::Skin;
use acacia_render::item::map;

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
        let sheet = Arc::new(map::skin(root, &picture.rgba)?);
        sheets.truncate(KEPT - 1);
        sheets.insert(0, (picture.clone(), sheet.clone()));
        Some(sheet)
    })
}
