//! The chat replays: the `hud` harness's `chat` kind (`tools/parity/harnesses/hud/cases.py`).

use serde_json::{Map, Value};

use acacia_ui::java::chat::{Chat, ChatOptions, Tag};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::Text;

use crate::Assets;

/// Messages added at their ticks (`Hud.tickCount`), maybe scrolled, then the chat at tick `now`:
/// in the background, or in the foreground when the chat screen is open (`focused`).
pub fn replay_hud(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let option = |k: &str, d: f64| c[k].as_f64().unwrap_or(d);
    let mut chat = Chat::new(ChatOptions {
        opacity: option("chatOpacity", 1.0),
        background_opacity: option("textOpacity", 0.5),
        scale: option("chatScale", 1.0),
        width: option("chatWidth", 1.0),
        height_focused: option("chatHeightFocused", 1.0),
        height_unfocused: option("chatHeightUnfocused", 70.0 / 160.0),
        line_spacing: option("chatLineSpacing", 0.0),
        colors: true,
    });
    let font = assets.font.as_ref();
    for m in c["messages"].as_array().into_iter().flatten() {
        let tag = match m["tag"].as_str() {
            Some("system") => Some(Tag::SYSTEM),
            Some("error") => Some(Tag::CHAT_ERROR),
            _ => None,
        };
        chat.add(font, Text::from_json(&m["text"], &|_| None), tag, m["at"].as_i64().unwrap_or(0) as i32, false);
    }
    let focused = c["focused"].as_bool().unwrap_or(false);
    if let Some(n) = c["scroll"].as_i64() {
        chat.scroll_chat(n as i32, focused);
    }
    let mut gui = Gui::new(c["width"].as_i64().unwrap_or(0) as i32, c["height"].as_i64().unwrap_or(0) as i32);
    chat.extract(&mut gui, c["now"].as_i64().unwrap_or(0) as i32, focused);
    gui.draws
}
