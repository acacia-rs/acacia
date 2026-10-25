//! The chat replays: the `hud` harness's `chat` kind (`tools/parity/harnesses/hud/cases.py`) and
//! the `chat_screen` harness.

use std::sync::Arc;

use serde_json::{Map, Value, json};

use acacia_ui::java::chat::{Chat, ChatAction, ChatLines, ChatOptions, ChatScreen, Completion, Key, Tag};
use acacia_ui::java::command::Tree;
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::mth::as8;
use acacia_ui::java::text::{Lang, Text};
use acacia_ui::java::tooltip::game::Game;
use acacia_ui::java::tooltip::item::Details;

use crate::Assets;

/// The case's chat: its options (`background` names the text background opacity's key) and
/// messages added at their ticks (`Hud.tickCount`) with their tags, text read through `lang`.
fn chat_from_case(c: &Value, assets: &Assets, background: &str, lang: Lang) -> Chat {
    let option = |k: &str, d: f64| c[k].as_f64().unwrap_or(d);
    let mut chat = Chat::new(ChatOptions {
        opacity: option("chatOpacity", 1.0),
        background_opacity: option(background, 0.5),
        scale: option("chatScale", 1.0),
        width: option("chatWidth", 1.0),
        height_focused: option("chatHeightFocused", 1.0),
        height_unfocused: option("chatHeightUnfocused", 70.0 / 160.0),
        line_spacing: option("chatLineSpacing", 0.0),
        colors: true,
    });
    let font = assets.font.as_ref();
    for m in c["messages"].as_array().into_iter().flatten() {
        // addClientSystemMessage tags as systemSinglePlayer.
        let tag = match m["tag"].as_str() {
            Some("system") => Some(Tag::SYSTEM_SINGLE_PLAYER),
            Some("error") => Some(Tag::CHAT_ERROR),
            Some("not_secure") => Some(Tag::CHAT_NOT_SECURE),
            Some("modified") => Some(Tag::modified(m["original"].as_str().unwrap_or_default())),
            _ => None,
        };
        chat.add(font, Text::from_json(&m["text"], lang), tag, int(m, "at"), false);
    }
    chat
}

/// Messages added at their ticks, maybe scrolled, then the chat at tick `now`: in the background,
/// or in the foreground when the chat screen is open (`focused`).
pub fn replay_hud(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let mut chat = chat_from_case(c, assets, "textOpacity", &|_| None);
    let focused = c["focused"].as_bool().unwrap_or(false);
    if let Some(n) = c["scroll"].as_i64() {
        chat.scroll_chat(n as i32, focused);
    }
    let mut gui = Gui::new(int(c, "width"), int(c, "height"));
    chat.extract(&mut gui, int(c, "now"), focused);
    gui.draws
}

fn int(c: &Value, k: &str) -> i32 {
    c[k].as_i64().unwrap_or(0) as i32
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)).collect()
}

/// A key by name after `shift+` and `ctrl+` modifiers.
fn key(name: &str) -> (Key, bool, bool) {
    let (mut name, mut shift, mut ctrl) = (name, false, false);
    loop {
        if let Some(n) = name.strip_prefix("shift+") {
            (shift, name) = (true, n);
        } else if let Some(n) = name.strip_prefix("ctrl+") {
            (ctrl, name) = (true, n);
        } else {
            break;
        }
    }
    let key = match name {
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "right" => Key::Right,
        "left" => Key::Left,
        "down" => Key::Down,
        "up" => Key::Up,
        "tab" => Key::Tab,
        "escape" => Key::Escape,
        _ => Key::A,
    };
    (key, shift, ctrl)
}

/// The `chat_screen` harness (`tools/parity/harnesses/chat_screen/cases.py`): the screen opened,
/// the actions replayed, then the input's background, the chat's lines, the input, the suggestions
/// and the hover tooltip drawn.
pub fn replay_screen(c: &Value, assets: &Assets, state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let m = assets.font.as_ref();
    let (w, h) = (int(c, "width"), int(c, "height"));
    let history = strings(&c["history"]);
    let lang = crate::kit::lang(assets);
    let mut completion = Completion::new(strings(&c["players"]), Arc::new(Tree::from_json(&c["commands"])), &lang);
    completion.auto_suggest = c["autoSuggest"].as_bool().unwrap_or(true);
    let mut chat = c.get("messages").map(|_| chat_from_case(c, assets, "opacity", &lang));
    let mut screen = ChatScreen::new(m, c["initial"].as_str().unwrap_or(""), history.len(), completion, w, h);
    if let (Some(chat), Some(n)) = (chat.as_mut(), c["scroll"].as_i64()) {
        chat.scroll_chat(n as i32, true);
    }
    let mouse = (c["mouse"][0].as_i64().unwrap_or(0) as i32, c["mouse"][1].as_i64().unwrap_or(0) as i32);
    for a in c["actions"].as_array().into_iter().flatten() {
        match a["type"].as_str().unwrap_or_default() {
            "scroll" => {
                screen.scroll(mouse, a["amount"].as_f64().unwrap_or(0.0), false);
            }
            "type" => a["text"].as_str().unwrap_or_default().chars().for_each(|ch| screen.char_typed(m, ch)),
            "click" => screen.click(m, a["x"].as_f64().unwrap_or(0.0), a["y"].as_f64().unwrap_or(0.0)),
            "key" => {
                let (k, shift, ctrl) = key(a["key"].as_str().unwrap_or_default());
                if screen.key(m, &history, k, shift, ctrl) == ChatAction::Close {
                    state.insert("closed".into(), json!(true));
                    return Vec::new();
                }
            }
            _ => {}
        }
    }
    // Options.getBackgroundColor(Integer.MIN_VALUE).
    let background = if c["chatOnly"].as_bool().unwrap_or(true) { 0x8000_0000 } else { as8(c["opacity"].as_f64().unwrap_or(0.0) as f32) << 24 };
    let mut gui = Gui::new(w, h);
    // No level or player facts: only plain items' tooltips are compared.
    let game = Game::default();
    let details = Details { lang: &lang, advanced: c["advanced"].as_bool().unwrap_or(false), creative: false, game: &game, player: None, key: None };
    let lines = chat.as_ref().map(|chat| ChatLines { chat, ticks: int(c, "now"), details: &details });
    screen.extract(&mut gui, m, mouse, c["blink"].as_u64().unwrap_or(0), background, lines);
    let input = &screen.input;
    state.insert("value".into(), json!(input.value()));
    state.insert("cursor".into(), json!(input.cursor));
    state.insert("highlight".into(), json!(input.highlight));
    state.insert("display".into(), json!(input.display));
    gui.draws
}
