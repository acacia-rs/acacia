use acacia_client::proto::packets::{
    ContainerClose, ContainerOpen, SetHealth, SetTitle, SetTitleType, Text, TextCategory, TextContent, TextContentRaw,
    TextType,
};
use acacia_client::proto::packets::OpenSign;
use acacia_client::proto::types::{BlockCoordinates, WindowID, WindowIDVarint, WindowType};
use acacia_client::proto::Packet;

use super::*;
use crate::state::queries::test_support::{content, empty_item, item, raw, slot_packet};
use crate::state::GameState;

fn run(source: &mut EventSource, state: &mut GameState, packet: RawPacket) -> Vec<BotEvent> {
    state.apply(&packet).unwrap();
    let mut out = EventOut::new(None);
    source.on_packet(&packet, state, &mut out);
    std::iter::from_fn(|| out.pop()).collect()
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
    let filled = content(WindowIDVarint::First, vec![item(5, 1, 9), empty_item()]);
    let events = run(&mut source, &mut state, raw(&filled));
    assert!(matches!(events.as_slice(), [BotEvent::WindowContents(c)] if c.slots.len() == 2));
    assert!(run(&mut source, &mut state, raw(&content(WindowIDVarint::Inventory, vec![item(5, 1, 9)]))).is_empty());
    assert!(run(&mut source, &mut state, raw(&slot_packet(WindowIDVarint::First, 0, empty_item()))).is_empty());
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

#[test]
fn entity_spawns_removals_and_moves_are_reported_when_wanted() {
    use acacia_client::proto::packets::{AddEntity, MoveEntityDelta, RemoveEntity};
    use crate::state::queries::test_support::fixtures;
    let mut add: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    (add.runtime_id, add.unique_id, add.entity_type) = (7, 70, "minecraft:zombie".into());
    let trackers = crate::state::Trackers { entities: true, ..Default::default() };
    let mut state = GameState::new(trackers, 1);
    let mut quiet = EventSource::new(Events::NONE, Vec::new());
    assert!(run(&mut quiet, &mut state, raw(&add)).is_empty());
    assert!(state.entities.changes.is_empty(), "changes are taken even when nobody wants them");

    let mut source = EventSource::new(Events::ENTITIES | Events::ENTITY_MOVES, Vec::new());
    (add.runtime_id, add.unique_id) = (8, 80);
    let events = run(&mut source, &mut state, raw(&add));
    assert!(matches!(events.as_slice(), [BotEvent::EntitySpawned(e)] if e.runtime_id == 8 && &*e.kind == "minecraft:zombie"));

    // As in Bot::run_ticks: the clock advances, then the moves are reported.
    let mut out = EventOut::new(None);
    state.entities.advance();
    source.on_ticks(&state, &mut out);
    assert!(matches!(out.pop(), Some(BotEvent::EntitiesMoved(m)) if m.len() == 2), "spawns count as moves");
    state.entities.advance();
    source.on_ticks(&state, &mut out);
    assert!(out.pop().is_none(), "nothing moved since");
    let mut delta: MoveEntityDelta = fixtures::<MoveEntityDelta>()[0].decode().unwrap();
    (delta.runtime_entity_id, delta.x) = (7, Some(3.0));
    run(&mut source, &mut state, raw(&delta));
    state.entities.advance();
    source.on_ticks(&state, &mut out);
    assert!(matches!(out.pop(), Some(BotEvent::EntitiesMoved(m)) if m.len() == 1 && m[0].runtime_id == 7));

    let events = run(&mut source, &mut state, raw(&RemoveEntity { entity_id_self: 70 }));
    assert!(matches!(events.as_slice(), [BotEvent::EntityGone(e)] if e.runtime_id == 7 && e.unique_id == 70));
}

#[tokio::test]
async fn a_full_channel_drops_only_lossy_events_and_counts_them() {
    let (sink, mut rx) = channel(2);
    [BotEvent::Tick, BotEvent::Tick, BotEvent::Tick, BotEvent::Tick, BotEvent::Died, BotEvent::Tick].into_iter().for_each(|e| sink(e));
    let got: Vec<String> = std::iter::from_fn(|| rx.try_recv()).map(|e| format!("{e:?}")).collect();
    assert_eq!(got, ["Tick", "Tick", "Lagged { dropped: 2 }", "Died", "Lagged { dropped: 1 }"]);
    sink(BotEvent::Respawned);
    drop(sink);
    assert!(matches!(rx.recv().await, Some(BotEvent::Respawned)), "queued events outlive the sink");
    assert!(rx.recv().await.is_none(), "then the receiver ends");
}
