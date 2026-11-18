use super::*;
use crate::item::typed::{DyedColor, Lore, PotionContents};
use crate::item::{Components, ItemStack};
use crate::server::round_trip;
use crate::text::{Component, NumberFormat};
use crate::types::ObjectiveRenderType;
use crate::{Error, VarInt, Wire, nbt};

fn text(s: &str) -> Component {
    Component::text(s)
}

fn plain(s: &str) -> Vec<u8> {
    let mut out = vec![nbt::TAG_STRING];
    out.extend((s.len() as u16).to_be_bytes());
    out.extend(s.as_bytes());
    out
}

fn bytes<T: Wire>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.write(&mut out);
    out
}

#[test]
fn boss_operations_match_the_codec() {
    let flags = BossFlags { darken_screen: true, play_music: false, create_world_fog: true };
    let add = BossEvent {
        id: 5,
        operation: BossOperation::Add { name: text("Wither"), progress: 0.5, color: BossColor::Yellow, overlay: BossOverlay::Notched12, flags },
    };
    let wire = round_trip(add);
    assert_eq!(wire[..16], 5u128.to_be_bytes());
    assert_eq!(wire[16], 0, "add");
    assert_eq!(wire[wire.len() - 3..], [4, 3, 0b101], "yellow, notched 12, darken and fog");

    assert_eq!(round_trip(BossEvent { id: 5, operation: BossOperation::Remove })[16..], [1]);
    assert_eq!(round_trip(BossEvent { id: 5, operation: BossOperation::UpdateProgress(0.25) })[16..], [2, 0x3e, 0x80, 0, 0]);
    round_trip(BossEvent { id: 5, operation: BossOperation::UpdateName(text("Dragon")) });
    assert_eq!(round_trip(BossEvent { id: 5, operation: BossOperation::UpdateStyle { color: BossColor::Yellow, overlay: BossOverlay::Notched12 } })[16..], [4, 4, 3]);
    assert_eq!(round_trip(BossEvent { id: 5, operation: BossOperation::UpdateProperties(flags) })[16..], [5, 0b101]);
    assert_eq!(BossEvent::read(&mut &[&[0u8; 16][..], &[6]].concat()[..]), Err(Error::Invalid("boss event operation")));
}

#[test]
fn objectives_carry_an_optional_number_format() {
    let add = SetObjective {
        objective: "kills".into(),
        method: ObjectiveMethod::Add(ObjectiveDisplay { name: text("Kills"), render_type: ObjectiveRenderType::Hearts, number_format: None }),
    };
    let wire = round_trip(add);
    assert_eq!(wire[..7], [5, b'k', b'i', b'l', b'l', b's', 0], "name, add");
    assert_eq!(wire[wire.len() - 2..], [1, 0], "hearts, no number format");
    assert_eq!(round_trip(SetObjective { objective: "kills".into(), method: ObjectiveMethod::Remove })[6..], [1]);
    round_trip(SetObjective {
        objective: "k".into(),
        method: ObjectiveMethod::Change(ObjectiveDisplay { name: text("K"), render_type: ObjectiveRenderType::Integer, number_format: Some(NumberFormat::Fixed(text("x"))) }),
    });
}

#[test]
fn scores_and_display_slots() {
    let score = SetScore { owner: "Bob".into(), objective: "kills".into(), score: VarInt(-2), display: OptionalText(None), number_format: None };
    let wire = round_trip(score);
    assert_eq!(wire[wire.len() - 7..], [0xfe, 0xff, 0xff, 0xff, 0x0f, 0, 0], "score -2, no display, no format");
    round_trip(SetScore {
        owner: "Bob".into(),
        objective: "kills".into(),
        score: VarInt(3),
        display: OptionalText(Some(text("Alice"))),
        number_format: Some(NumberFormat::Blank),
    });

    assert_eq!(round_trip(ResetScore { owner: "Bob".into(), objective: NullableString(Some("kills".into())) })[4..], [1, 5, b'k', b'i', b'l', b'l', b's']);
    assert_eq!(round_trip(ResetScore { owner: "Bob".into(), objective: NullableString(None) })[4..], [0]);

    assert_eq!(round_trip(SetDisplayObjective { slot: DisplaySlot::Sidebar, objective: "kills".into() })[..2], [1, 5]);
    assert_eq!(DisplaySlot::Team(TeamColor::Red).id(), 15);
    assert_eq!(DisplaySlot::read(&mut &[15u8][..]), Ok(DisplaySlot::Team(TeamColor::Red)));
    assert_eq!(DisplaySlot::read(&mut &[19u8][..]), Err(Error::Invalid("display slot")));
}

#[test]
fn teams_by_method() {
    let parameters = TeamParameters {
        display_name: text("Red"),
        prefix: text("[R] "),
        suffix: text(""),
        name_tag_visibility: NameTagVisibility::HideForOtherTeams,
        collision_rule: CollisionRule::PushOwnTeam,
        color: OptionalTeamColor(Some(TeamColor::Red)),
        options: 3,
    };
    let players = vec!["a".to_string(), "b".to_string()];
    let add = round_trip(SetPlayerTeam { team: "red".into(), action: TeamAction::Add { parameters: parameters.clone(), players: players.clone() } });
    assert_eq!(add[4], 0);
    assert_eq!(round_trip(SetPlayerTeam { team: "red".into(), action: TeamAction::Remove })[4..], [1]);
    assert_eq!(round_trip(SetPlayerTeam { team: "red".into(), action: TeamAction::Change(parameters.clone()) })[4], 2);
    assert_eq!(round_trip(SetPlayerTeam { team: "red".into(), action: TeamAction::Join(players.clone()) })[4..], [3, 2, 1, b'a', 1, b'b']);
    assert_eq!(round_trip(SetPlayerTeam { team: "red".into(), action: TeamAction::Leave(players) })[4], 4);
    let tail = bytes(&parameters);
    assert_eq!(tail[tail.len() - 5..], [2, 3, 1, 12, 3], "visibility, collision, colour present + red, options");
}

#[test]
fn hud_packets_are_plain_layouts() {
    assert_eq!(round_trip(SetTitlesAnimation { fade_in: 10, stay: 70, fade_out: 20 }), [0, 0, 0, 10, 0, 0, 0, 70, 0, 0, 0, 20]);
    assert_eq!(round_trip(ClearTitles { reset_times: true }), [1]);
    assert_eq!(round_trip(SetActionBarText { text: text("hi") }), plain("hi"));
    assert_eq!(round_trip(SetTitleText { text: text("t") }), plain("t"));
    assert_eq!(round_trip(SetSubtitleText { text: text("s") }), plain("s"));
    assert_eq!(round_trip(SystemChat { content: text("x"), overlay: true }), [plain("x"), vec![1]].concat());
    assert_eq!(round_trip(TabList { header: text("h"), footer: text("f") }), [plain("h"), plain("f")].concat());
    assert_eq!(round_trip(SetExperience { progress: 0.5, level: VarInt(300), total: VarInt(1) }), [0x3f, 0, 0, 0, 0xac, 0x02, 1], "progress, level, total");
    assert_eq!(round_trip(SetHealth { health: 20.0, food: VarInt(18), saturation: 5.0 }).len(), 9);
}

fn last_seen() -> LastSeenUpdate {
    LastSeenUpdate { offset: 2, acknowledged: [1, 2, 3], checksum: 9 }
}

#[test]
fn serverbound_chat_matches_the_vanilla_layout() {
    let signature = MessageSignature([7; SIGNATURE_BYTES]);
    let signed = SignedChat { message: "hello".into(), timestamp_millis: 1_000, salt: -5, signature: Some(signature.clone()), last_seen: last_seen() };
    let wire = round_trip(signed);
    assert_eq!(wire.len(), 1 + 5 + 8 + 8 + 1 + 256 + 1 + 3 + 1);
    let unsigned = SignedChat { message: "hi".into(), timestamp_millis: 0, salt: 0, signature: None, last_seen: LastSeenUpdate::default() };
    round_trip(unsigned.clone());

    // The crate's own unsigned encoder is a valid signed-chat frame with no signature.
    let ours = bytes(&UnsignedChat { message: "hi".into(), timestamp_millis: 0, salt: 0 });
    assert_eq!(SignedChat::read(&mut &ours[..]), Ok(unsigned));

    let long = bytes(&"x".repeat(MAX_MESSAGE + 1));
    assert_eq!(SignedChat::read(&mut &long[..]), Err(Error::Invalid("string longer than its cap")));
    let emoji = bytes(&"😀".repeat(MAX_MESSAGE / 2 + 1));
    assert!(SignedChat::read(&mut &emoji[..]).is_err(), "the cap counts UTF-16 units");
}

#[test]
fn signed_commands_cap_their_argument_signatures() {
    let argument = |name: &str| ArgumentSignature { name: name.into(), signature: MessageSignature([1; SIGNATURE_BYTES]) };
    let command = ChatCommandSigned {
        command: "msg bob hi".into(),
        timestamp_millis: 77,
        salt: 3,
        arguments: vec![argument("message")],
        last_seen: last_seen(),
    };
    let wire = round_trip(command.clone());
    assert_eq!(wire[0], 10);
    let too_many = ChatCommandSigned { arguments: vec![argument("a"); MAX_ARGUMENT_SIGNATURES + 1], ..command.clone() };
    assert_eq!(ChatCommandSigned::read(&mut &bytes(&too_many)[..]), Err(Error::Invalid("list longer than its cap")));
    let long_name = ChatCommandSigned { arguments: vec![argument(&"n".repeat(17))], ..command };
    assert!(ChatCommandSigned::read(&mut &bytes(&long_name)[..]).is_err());
}

#[test]
fn session_updates_cap_the_key_and_its_signature() {
    let update = ChatSessionUpdate { session_id: 9, expires_at_millis: 5, public_key: vec![1; 294], key_signature: vec![2; 512] };
    assert_eq!(round_trip(update.clone()).len(), 16 + 8 + 2 + 294 + 2 + 512);
    let big_key = ChatSessionUpdate { public_key: vec![0; MAX_PUBLIC_KEY + 1], ..update.clone() };
    assert!(ChatSessionUpdate::read(&mut &bytes(&big_key)[..]).is_err());
    let big_signature = ChatSessionUpdate { key_signature: vec![0; MAX_KEY_SIGNATURE + 1], ..update };
    assert!(ChatSessionUpdate::read(&mut &bytes(&big_signature)[..]).is_err());
    assert_eq!(round_trip(ChatAck { offset: VarInt(4) }), [4]);
}

#[test]
fn relayed_chat_round_trips() {
    let bound = ChatTypeBound { chat_type: 0, name: text("Bob"), target: OptionalText(Some(text("Al"))) };
    assert_eq!(bytes(&bound)[0], 1, "holder id + 1");
    assert_eq!(ChatTypeBound::read(&mut &[0u8][..]), Err(Error::Invalid("inline chat type")));
    let body = SignedBody {
        content: "hey".into(),
        timestamp_millis: 99,
        salt: 1,
        last_seen: vec![PackedSignature::Cached(3), PackedSignature::Full(MessageSignature([9; SIGNATURE_BYTES]))],
    };
    let chat = PlayerChat {
        global_index: VarInt(12),
        sender: 0xabc,
        index: VarInt(2),
        signature: None,
        body: body.clone(),
        unsigned_content: OptionalText(Some(text("hey!"))),
        filter_mask: FilterMask::Partially(vec![0b101]),
        chat_type: bound.clone(),
    };
    round_trip(chat);
    round_trip(PlayerChat {
        global_index: VarInt(0),
        sender: 1,
        index: VarInt(0),
        signature: Some(MessageSignature([4; SIGNATURE_BYTES])),
        body,
        unsigned_content: OptionalText(None),
        filter_mask: FilterMask::FullyFiltered,
        chat_type: bound.clone(),
    });
    assert_eq!(round_trip(DisguisedChat { message: text("m"), chat_type: bound })[..plain("m").len()], plain("m")[..]);
    assert_eq!(round_trip(DeleteChat { signature: PackedSignature::Cached(0) }), [1]);
    assert_eq!(bytes(&PackedSignature::Full(MessageSignature([0; SIGNATURE_BYTES])))[..2], [0, 0]);
    assert_eq!(bytes(&FilterMask::PassThrough), [0]);
}

fn literal(text: &str, children: &[i32]) -> CommandNode {
    CommandNode { children: children.to_vec(), ..CommandNode::new(NodeKind::Literal(text.into())) }
}

#[test]
fn command_tree_matches_the_bridges_hand_encoding() {
    let argument = |name: &str, parser| CommandNode { executable: true, ..CommandNode::new(NodeKind::Argument { name: name.into(), parser, suggestions: None }) };
    let tree = Commands {
        nodes: vec![
            CommandNode { children: vec![1, 3], ..CommandNode::new(NodeKind::Root) },
            CommandNode { executable: true, children: vec![2], ..literal("tp", &[]) },
            argument("target", Parser::Entity { single: false, players_only: false }),
            CommandNode { redirect: Some(1), ..literal("teleport", &[]) },
        ],
        root: 0,
    };
    let wire = round_trip(tree);
    let mut expected = vec![4, 0, 2, 1, 3];
    expected.extend([1 | 4, 1, 2, 2, b't', b'p']);
    expected.extend([2 | 4, 0, 6, b't', b'a', b'r', b'g', b'e', b't', 6, 0]);
    expected.extend([1 | 8, 0, 1, 8, b't', b'e', b'l', b'e', b'p', b'o', b'r', b't']);
    expected.push(0);
    assert_eq!(wire, expected, "nodes, then the root index");
}

#[test]
fn parsers_use_the_registry_ids_and_option_layouts() {
    let cases: [(Parser, &[u8]); 10] = [
        (Parser::Float { min: None, max: None }, &[1, 0]),
        (Parser::Integer { min: Some(1), max: Some(9) }, &[3, 3, 0, 0, 0, 1, 0, 0, 0, 9]),
        (Parser::Integer { min: None, max: None }, &[3, 0]),
        (Parser::String(StringKind::SingleWord), &[5, 0]),
        (Parser::String(StringKind::Quotable), &[5, 1]),
        (Parser::String(StringKind::Greedy), &[5, 2]),
        (Parser::Entity { single: true, players_only: true }, &[6, 3]),
        (Parser::Vec3, &[10]),
        (Parser::Message, &[20]),
        (Parser::Long { min: None, max: Some(5) }, &[4, 2, 0, 0, 0, 0, 0, 0, 0, 5]),
    ];
    for (parser, expected) in cases {
        assert_eq!(round_trip(parser.clone()), expected, "{parser:?}");
    }
    assert!(Parser::read(&mut &[41u8][..]).is_err(), "no known layout for int_range");
}

#[test]
fn argument_suggestions_and_restriction_are_flagged() {
    let node = CommandNode {
        restricted: true,
        ..CommandNode::new(NodeKind::Argument { name: "n".into(), parser: Parser::Bool, suggestions: Some("minecraft:ask_server".into()) })
    };
    let wire = round_trip(node);
    assert_eq!(wire[0], 2 | 16 | 32);
}

#[test]
fn container_bodies_round_trip_with_typed_items() {
    let mut components = Components::default().with(&Lore(vec![text("a")])).with(&DyedColor(0xff));
    components.damage = Some(5);
    let item = ItemStack { item: 800, count: 2, components };
    let content = ContainerSetContent {
        container: VarInt(3),
        state_id: VarInt(1),
        items: vec![Some(item.clone()), None],
        carried: Some(ItemStack { components: Components::default().with(&PotionContents { potion: Some(1), ..PotionContents::default() }), ..item.clone() }),
    };
    round_trip(content);
    round_trip(ContainerSetSlot { container: VarInt(3), state_id: VarInt(1), slot: 4, item: Some(item.clone()) });
    round_trip(SetCursorItem { item: Some(item.clone()) });
    round_trip(SetPlayerInventory { slot: VarInt(40), item: Some(item) });
    assert_eq!(round_trip(SetHeldSlot { slot: VarInt(8) }), [8]);
    assert_eq!(round_trip(ContainerClose { container: VarInt(3) }), [3], "one body both ways");
    assert_eq!(round_trip(ContainerSetData { container: VarInt(3), id: 1, value: -1 }), [3, 0, 1, 0xff, 0xff]);
    assert_eq!(round_trip(SetCarriedItem { slot: 8 }), [0, 8]);
    assert_eq!(round_trip(ContainerSlotStateChanged { slot: VarInt(2), container: VarInt(5), new_state: true }), [2, 5, 1]);
    assert_eq!(round_trip(PlaceGhostRecipe { container: VarInt(2), display: vec![1, 2, 3] }), [2, 1, 2, 3]);
}

