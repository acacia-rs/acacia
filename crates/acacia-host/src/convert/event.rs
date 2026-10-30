use acacia_api::{
    BlockPos, ChatKind, ChatLine, EndReason, Event, Form, FormBody, FormButton, FormElement, TitleKind, UniqueId, Window,
};
use acacia_bot::events::{ChatKind as BotChatKind, ChatMessage, Title, TitleKind as BotTitleKind};
use acacia_bot::forms::{self, Element, FormKind};
use acacia_bot::state::Container;
use acacia_bot::{BotEvent, DisconnectReason};

use super::snapshot::{entity, player, slots};

/// `None` for what the schema does not carry yet (raw packets).
pub(crate) fn event(e: BotEvent) -> Option<Event> {
    Some(match e {
        BotEvent::Chat(m) => Event::Chat(chat(m)),
        BotEvent::ChatMatch(m) => Event::ChatMatch { pattern: m.pattern, groups: m.groups, line: chat(m.message) },
        BotEvent::Form(f) => Event::Form(form(f)),
        BotEvent::Health { health, food, saturation } => Event::Health { health, food, saturation },
        BotEvent::Died => Event::Death,
        BotEvent::Respawned => Event::Respawn,
        BotEvent::PlayerJoined(p) => Event::PlayerJoined(player(&p)),
        BotEvent::PlayerLeft(p) => Event::PlayerLeft(player(&p)),
        BotEvent::EntitySpawned(e) => Event::EntitySpawn(entity(&e)),
        BotEvent::EntityGone(e) => Event::EntityGone(entity(&e)),
        BotEvent::EntitiesMoved(moved) => Event::EntityMoved { entities: moved.iter().map(entity).collect() },
        BotEvent::WindowOpened(c) => Event::WindowOpen(window(&c)),
        BotEvent::WindowContents(c) => Event::WindowUpdate(window(&c)),
        BotEvent::WindowClosed { window_id } => Event::WindowClose { window_id },
        BotEvent::SignEditor { position, front } => Event::SignEditor { position: position.into(), front },
        BotEvent::Title(t) => title(t),
        BotEvent::Slept => Event::Sleep,
        BotEvent::Woke => Event::Wake,
        BotEvent::Tick => Event::PhysicsTick,
        BotEvent::Violation(v) => Event::Violation { message: v.to_string() },
        BotEvent::Lagged { dropped } => Event::Lagged { dropped },
        BotEvent::Disconnected(reason) => end(&reason),
        _ => return None,
    })
}

fn chat(m: ChatMessage) -> ChatLine {
    let kind = match m.kind {
        BotChatKind::Chat => ChatKind::Chat,
        BotChatKind::Whisper => ChatKind::Whisper,
        BotChatKind::Announcement => ChatKind::Announcement,
        BotChatKind::Raw => ChatKind::Raw,
        BotChatKind::Translation => ChatKind::Translation,
        BotChatKind::Popup => ChatKind::Popup,
        BotChatKind::JukeboxPopup => ChatKind::JukeboxPopup,
        BotChatKind::Tip => ChatKind::Tip,
        BotChatKind::System => ChatKind::System,
    };
    let plain = m.plain();
    ChatLine { kind, sender: m.sender, message: m.message, plain, params: m.params, rawtext: m.rawtext, xuid: m.xuid }
}

fn title(t: Title) -> Event {
    let kind = match t.kind {
        BotTitleKind::Title => TitleKind::Title,
        BotTitleKind::Subtitle => TitleKind::Subtitle,
        BotTitleKind::ActionBar => TitleKind::ActionBar,
        BotTitleKind::Clear => TitleKind::Clear,
    };
    Event::Title { kind, text: t.text }
}

pub(crate) fn end(reason: &DisconnectReason) -> Event {
    let (reason, message) = match reason {
        DisconnectReason::LocalClose => (EndReason::Closed, "closed".to_owned()),
        DisconnectReason::Kicked { message, .. } => (EndReason::Kicked, message.clone()),
        DisconnectReason::Transfer { address, port } => (EndReason::Transfer, format!("{address}:{port}")),
        DisconnectReason::LoginFailed(status) => (EndReason::LoginFailed, format!("{status:?}")),
        other => (EndReason::Lost, format!("{other:?}")),
    };
    Event::End { reason, message }
}

fn window(c: &Container) -> Window {
    Window {
        window_id: c.window_id,
        window_type: format!("{:?}", c.window_type),
        position: c.position.as_ref().map(|p| BlockPos { x: p.x, y: p.y, z: p.z }),
        entity: c.entity.map(UniqueId),
        title: c.title.clone(),
        slots: slots(c.window_id, &c.slots),
    }
}

fn form(f: forms::Form) -> Form {
    let body = match f.kind {
        FormKind::Simple { content, buttons, elements, positions } => FormBody::Simple {
            content,
            buttons: buttons.into_iter().map(|b| FormButton { text: b.text, image: b.image }).collect(),
            elements: elements.into_iter().map(element).collect(),
            positions: positions.into_iter().map(|p| p as u32).collect(),
        },
        FormKind::Modal { content, yes, no } => FormBody::Modal { content, yes, no },
        FormKind::Custom { elements } => FormBody::Custom { elements: elements.into_iter().map(element).collect() },
    };
    Form { id: f.id, title: f.title, body }
}

fn element(e: Element) -> FormElement {
    match e {
        Element::Label { text } => FormElement::Label { text },
        Element::Header { text } => FormElement::Header { text },
        Element::Divider => FormElement::Divider,
        Element::Input { text, placeholder, default } => FormElement::Input { text, placeholder, default },
        Element::Toggle { text, default } => FormElement::Toggle { text, default },
        Element::Slider { text, min, max, step, default } => FormElement::Slider { text, min, max, step, default },
        Element::StepSlider { text, steps, default } => FormElement::StepSlider { text, steps, default: default as u32 },
        Element::Dropdown { text, options, default } => FormElement::Dropdown { text, options, default: default as u32 },
    }
}
