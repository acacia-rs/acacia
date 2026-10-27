//! What a press on an options screen does: the page its button opens, Done, or the value an
//! option's widget sets (`CycleButton.cycleValue`, `OptionInstanceSliderButton.applyValue`).

use std::borrow::Cow;

use super::defs::def;
use super::pages::DONE;
use super::value::{Label, V, Values, value};
use crate::java::widget::input::{Action, Fired};

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A page (as `pages::build` names it) opens over this one.
    Open(&'static str),
    /// Done: back to the screen this one was opened from.
    Done,
    /// A button whose screen is not ported.
    Pressed(Cow<'static, str>),
    /// Option `id` takes this value.
    Set(Cow<'static, str>, V),
}

/// The page a button opens, for the screens that are ported (the online-service and link screens
/// are not). The pages past `pages::build`'s are the screens beside the options: `credits`
/// (`CreditsAndAttributionScreen`), `win` (its credits roll), `keybinds`, `language`, `packs` and
/// `telemetry`.
pub fn link(id: &str) -> Option<&'static str> {
    Some(match id {
        "options.skinCustomisation" => "skin",
        "options.sounds" => "sound",
        "options.video" => "video",
        "options.controls" => "controls",
        "options.chat" => "chat",
        "options.accessibility" => "accessibility",
        "options.online" => "online",
        "options.mouse_settings" => "mouse",
        "options.credits_and_attribution" => "credits",
        crate::java::screen::credits::CREDITS => "win",
        "controls.keybinds" => "keybinds",
        "options.language" => "language",
        "options.resourcepack" => "packs",
        "options.telemetry" => "telemetry",
        _ => return None,
    })
}

/// The values option `id`'s `CycleButton` steps through; `max_gui_scale` is
/// `Window.calculateScale(0, forceUnicode)`.
pub fn cycle_values(id: &str, max_gui_scale: i32) -> Vec<V> {
    let d = def(id);
    match (id, d.label, &d.default) {
        ("guiScale", ..) => (0..=max_gui_scale.max(0)).map(V::Int).collect(),
        (_, Label::Keys(keys), _) => keys.iter().map(|k| V::Name(k.0.to_owned())).collect(),
        (.., V::Bool(_)) => vec![V::Bool(true), V::Bool(false)],
        _ => vec![d.default.clone()],
    }
}

/// `cycleValue(delta)`: from the current value's index (-1 when it is not among them).
pub fn cycled(values: Values, id: &str, back: bool, max_gui_scale: i32) -> V {
    let list = cycle_values(id, max_gui_scale);
    let current = value(values, id);
    let index = list.iter().position(|v| *v == current).map_or(-1, |i| i as i64);
    let n = list.len() as i64;
    list[(index + if back { -1 } else { 1 }).rem_euclid(n) as usize].clone()
}

/// What `fired` on an options screen does.
pub fn event(values: Values, max_gui_scale: i32, fired: &Fired) -> Event {
    let id = &fired.id;
    if id == DONE {
        return Event::Done;
    }
    if let Some(page) = link(id) {
        return Event::Open(page);
    }
    match fired.action {
        Action::Press => Event::Pressed(id.clone()),
        Action::Cycle { back } => Event::Set(id.clone(), cycled(values, id, back, max_gui_scale)),
        Action::Slide(s) => Event::Set(id.clone(), def(id).set.from_slider(s)),
    }
}
