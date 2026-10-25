//! The `stations` harness's kinds; the case schema is `tools/parity/harnesses/stations/cases.py`'s,
//! with `ScreenKit`'s resolved `items` and the menu's final data slots (`game.data`).

use serde_json::{Map, Value};

use acacia_ui::java::gui::GuiDraw;

use crate::Assets;

pub fn replay(_c: &Value, _assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    Vec::new()
}
