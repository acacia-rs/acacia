use acacia_client::proto::packets::{
    ContainerClose, ContainerOpen, SetHealth, SetTitle, SetTitleType, Text, TextCategory, TextContent, TextContentRaw,
    TextType,
};
use acacia_client::proto::types::{BlockCoordinates, WindowID, WindowType};

use super::*;
use crate::state::queries::test_support::raw;

fn run(source: &mut EventSource, state: &mut GameState, packet: RawPacket) -> Vec<BotEvent> {
    state.apply(&packet).unwrap();
    let mut out = VecDeque::new();
    source.on_packet(&packet, state, &mut out);
    out.into()
}

fn chat_line(message: &str) -> RawPacket {
    raw(&Text {
        needs_translation: false,
        category: TextCategory::MessageOnly,
        r#type: TextType::Raw,
        content: TextContent::Raw(TextContentRaw { message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    })
}

#[test]
fn flags_combine() {
    let e = Events::CHAT | Events::HEALTH;
    assert!(e.contains(Events::CHAT) && e.contains(Events::HEALTH) && !e.contains(Events::FORMS));
    assert!(!Events::ALL.contains(Events::NONE));
    assert!(Events::ALL.contains(Events::TITLES));
}

#[test]
fn chat_needs_its_flag_but_patterns_run_without_it() {
    let mut state = GameState::default();
    let pattern = ChatPattern::new("pay", r"^(\w+) paid you \$(\d+)$").unwrap();
    let mut source = EventSource::new(Events::NONE, vec![pattern]);
    assert_eq!(source.packet_ids(), vec![Text::ID]);
    let events = run(&mut source, &mut state, chat_line("§aAlex paid you $50"));
    let [BotEvent::ChatMatch(m)] = events.as_slice() else { panic!("{events:?}") };
    assert_eq!((m.pattern.as_str(), m.groups.clone()), ("pay", vec!["Alex".to_owned(), "50".to_owned()]));

    let mut source = EventSource::new(Events::CHAT, Vec::new());
    let events = run(&mut source, &mut state, chat_line("hello"));
    assert!(matches!(events.as_slice(), [BotEvent::Chat(m)] if m.message == "hello"));
    assert!(run(&mut EventSource::new(Events::NONE, Vec::new()), &mut state, chat_line("x")).is_empty());
}

#[test]
fn death_and_respawn() {
    let mut state = GameState::default();
    let mut source = EventSource::new(Events::HEALTH, Vec::new());
    let events = run(&mut source, &mut state, raw(&SetHealth { health: 0 }));
    assert!(matches!(events.as_slice(), [BotEvent::Health { health, .. }, BotEvent::Died] if *health == 0.0));
    assert!(run(&mut source, &mut state, raw(&SetHealth { health: 0 })).is_empty());
    let events = run(&mut source, &mut state, raw(&SetHealth { health: 20 }));
    assert!(matches!(events.as_slice(), [BotEvent::Health { .. }, BotEvent::Respawned]));
}

#[test]
fn windows_open_and_close() {
    let mut state = GameState::default();
    let mut source = EventSource::new(Events::WINDOWS, Vec::new());
    let open = ContainerOpen {
        window_id: WindowID::First,
        window_type: WindowType::Furnace,
        coordinates: BlockCoordinates { x: 1, y: 2, z: 3 },
        runtime_entity_id: -1,
    };
    let events = run(&mut source, &mut state, raw(&open));
    assert!(matches!(events.as_slice(), [BotEvent::WindowOpened(c)] if c.window_type == WindowType::Furnace));
    let close = ContainerClose { window_id: WindowID::First, window_type: WindowType::Furnace, server: true };
    let events = run(&mut source, &mut state, raw(&close));
    assert!(matches!(events.as_slice(), [BotEvent::WindowClosed { window_id: 1 }]));
}

#[test]
fn titles_flatten_json() {
    let mut state = GameState::default();
    let mut source = EventSource::new(Events::TITLES, Vec::new());
    let title = |r#type, text: &str| {
        raw(&SetTitle {
            r#type,
            text: text.into(),
            fade_in_time: 0,
            stay_time: 0,
            fade_out_time: 0,
            xuid: String::new(),
            platform_online_id: String::new(),
            filtered_message: String::new(),
        })
    };
    let events = run(&mut source, &mut state, title(SetTitleType::ActionBarMessageJson, r#"{"rawtext":[{"text":"Balance: 5"}]}"#));
    assert!(matches!(events.as_slice(), [BotEvent::Title(t)] if t.kind == TitleKind::ActionBar && t.text == "Balance: 5"));
    assert!(run(&mut source, &mut state, title(SetTitleType::SetDurations, "")).is_empty());
}

#[test]
fn sleeping_and_sign_editor() {
    use acacia_client::proto::packets::SetEntityData;
    use acacia_client::proto::types::{
        EntityProperties, MetadataDictionaryItem, MetadataDictionaryItemKey, MetadataDictionaryItemType,
        MetadataDictionaryItemValue, MetadataFlags2,
    };
    let sleeping = |on: bool| {
        let flags = if on { MetadataFlags2::SLEEPING } else { MetadataFlags2::default() };
        let item = MetadataDictionaryItem {
            key: MetadataDictionaryItemKey::FlagsExtended,
            r#type: MetadataDictionaryItemType::Long,
            legacy_type: 0,
            value: MetadataDictionaryItemValue::FlagsExtended(flags),
        };
        raw(&SetEntityData { runtime_entity_id: 0, metadata: vec![item], properties: EntityProperties { ints: vec![], floats: vec![] }, tick: 0 })
    };
    let mut state = GameState::default();
    let mut source = EventSource::new(Events::SLEEP | Events::WINDOWS, Vec::new());
    assert!(matches!(run(&mut source, &mut state, sleeping(true)).as_slice(), [BotEvent::Slept]));
    assert!(run(&mut source, &mut state, sleeping(true)).is_empty());
    assert!(matches!(run(&mut source, &mut state, sleeping(false)).as_slice(), [BotEvent::Woke]));

    let open = OpenSign { position: BlockCoordinates { x: 4, y: 5, z: 6 }, is_front: true };
    let events = run(&mut source, &mut state, raw(&open));
    assert!(matches!(events.as_slice(), [BotEvent::SignEditor { position: [4, 5, 6], front: true }]));
}
