use acacia_client::proto::packets::{
    PlayerAction, SetEntityData, SetHealth, Text, TextCategory, TextContent, TextContentJukeboxPopup, TextContentRaw,
    TextContentTranslation, TextType,
};
use acacia_client::proto::types::{
    Action, EntityProperties, MetadataDictionaryItem, MetadataDictionaryItemKey as Key, MetadataDictionaryItemType as Type,
    MetadataDictionaryItemValue as Value, MetadataDictionaryItemValueDefault, MetadataFlags2,
};
use acacia_client::proto::RawPacket;

use super::{bed_refusal, sleep_action};
use crate::state::queries::test_support::raw;
use crate::state::GameState;

fn text(r#type: TextType, content: TextContent) -> RawPacket {
    raw(&Text {
        needs_translation: matches!(r#type, TextType::Translation),
        category: TextCategory::MessageOnly,
        r#type,
        content,
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    })
}

fn translation(key: &str) -> RawPacket {
    text(TextType::Translation, TextContent::Translation(TextContentTranslation { message: key.into(), parameters: Vec::new() }))
}

#[test]
fn recognises_bed_refusals() {
    assert_eq!(bed_refusal(&translation("§7%tile.bed.noSleep")).as_deref(), Some("%tile.bed.noSleep"));
    assert!(bed_refusal(&translation("%tile.bed.notSafe")).is_some());
    assert!(bed_refusal(&translation("%tile.bed.occupied")).is_some());
    let popup = TextContent::JukeboxPopup(TextContentJukeboxPopup {
        message: "You can sleep only at night or during thunderstorms".into(),
        parameters: Vec::new(),
    });
    assert!(bed_refusal(&text(TextType::JukeboxPopup, popup)).is_some());
    assert!(bed_refusal(&translation("%tile.bed.respawnSet")).is_none(), "success message");
    let chat = TextContent::Raw(TextContentRaw { message: "<Alex> beds are cool".into() });
    assert!(bed_refusal(&text(TextType::Raw, chat)).is_none());
    assert!(bed_refusal(&raw(&SetHealth { health: 3 })).is_none());
}

#[test]
fn bed_actions_have_zero_fields_like_vanilla() {
    for action in [Action::StartSleeping, Action::StopSleeping] {
        let p: PlayerAction = raw(&sleep_action(9, action)).decode().unwrap();
        assert_eq!((p.runtime_entity_id, p.action, p.face), (9, action, 0));
        assert_eq!((p.position.x, p.position.y, p.position.z), (0, 0, 0));
        assert_eq!((p.result_position.x, p.result_position.y, p.result_position.z), (0, 0, 0));
    }
}

fn entity_data(runtime_id: u64, items: Vec<(Key, Type, Value)>) -> RawPacket {
    let metadata = items.into_iter().map(|(key, r#type, value)| MetadataDictionaryItem { key, r#type, legacy_type: 0, value }).collect();
    raw(&SetEntityData { runtime_entity_id: runtime_id, metadata, properties: EntityProperties { ints: vec![], floats: vec![] }, tick: 0 })
}

fn player_flags(flags: i8) -> (Key, Type, Value) {
    (Key::PlayerFlags, Type::Byte, Value::Default(MetadataDictionaryItemValueDefault::Byte(flags)))
}

fn extended(flags: MetadataFlags2) -> (Key, Type, Value) {
    (Key::FlagsExtended, Type::Long, Value::FlagsExtended(flags))
}

#[test]
fn sleeping_follows_either_flag() {
    let mut state = GameState::default();
    state.player.runtime_entity_id = 5;
    state.apply(&entity_data(6, vec![player_flags(2)])).unwrap();
    assert!(!state.player.is_sleeping(), "another entity");

    state.apply(&entity_data(5, vec![player_flags(2)])).unwrap();
    assert!(state.player.is_sleeping(), "BDS / PocketMine player flag");
    state.apply(&entity_data(5, vec![extended(MetadataFlags2::default())])).unwrap();
    assert!(state.player.is_sleeping(), "an unrelated flags update keeps the player flag");
    state.apply(&entity_data(5, vec![player_flags(0)])).unwrap();
    assert!(!state.player.is_sleeping());

    state.apply(&entity_data(5, vec![extended(MetadataFlags2::SLEEPING)])).unwrap();
    assert!(state.player.is_sleeping(), "Geyser entity flag");
    state.apply(&entity_data(5, vec![extended(MetadataFlags2::default())])).unwrap();
    assert!(!state.player.is_sleeping());
}

#[test]
fn sleeping_position_is_the_clicked_block_raised_by_the_bed_height() {
    use super::tick::lying_in;
    let at = |x, y, z| acacia_client::proto::types::Vec3f { x, y, z };
    // Live on BDS: clicking the scene's bed at (2, -60, 0) put the eye at (2.5, -59.09375, 0.5).
    let bed = lying_in("minecraft:bed", "direction=3,head_piece_bit=0,occupied_bit=0", [2, -60, 0]);
    assert_eq!(bed, Some((at(2.5, -60.0 + 0.90625, 0.5), -90.0)));
    let straw = lying_in("minecraft:straw_bed", "direction=0,head_piece_bit=0", [0, 64, 0]);
    assert_eq!(straw, Some((at(0.5, 64.59375, 0.5), 0.0)));
    assert_eq!(lying_in("minecraft:stone", "", [0, 64, 0]), None);
}
