use acacia_java_proto::tag::Tag;
use acacia_java_proto::{Wire, write_varint};

use super::*;

fn component(text: &str, out: &mut Vec<u8>) {
    Tag::String(text.into()).write_root(out);
}

fn types() -> Vec<String> {
    ["minecraft:chat", "minecraft:msg_command_incoming"].map(String::from).to_vec()
}

#[test]
fn system_chat_and_its_action_bar() {
    let mut body = Vec::new();
    component("Server restarting", &mut body);
    true.write(&mut body);
    let m = read(from::SYSTEM_CHAT, &body, &[], 1).unwrap().unwrap();
    assert_eq!((m.kind, m.text.as_str()), (MessageKind::ActionBar, "Server restarting"));
}

#[test]
fn signed_player_chat_with_last_seen_and_a_partial_filter() {
    let mut body = Vec::new();
    write_varint(7, &mut body);
    0xabcd_u128.write(&mut body);
    write_varint(0, &mut body);
    true.write(&mut body);
    body.extend([9; SIGNATURE_BYTES]);
    "hello there".to_owned().write(&mut body);
    1_700_000_000_000_i64.write(&mut body);
    42_i64.write(&mut body);
    write_varint(2, &mut body);
    write_varint(0, &mut body);
    body.extend([1; SIGNATURE_BYTES]);
    write_varint(5, &mut body);
    false.write(&mut body);
    write_varint(FILTER_PARTIAL, &mut body);
    write_varint(1, &mut body);
    3_i64.write(&mut body);
    write_varint(2, &mut body);
    component("Steve", &mut body);
    false.write(&mut body);
    let m = read(from::PLAYER_CHAT, &body, &types(), 1).unwrap().unwrap();
    assert_eq!(m.kind, MessageKind::Player);
    assert_eq!(m.chat_type.as_deref(), Some("minecraft:msg_command_incoming"));
    assert_eq!((m.sender.as_deref(), m.sender_uuid, m.text.as_str()), (Some("Steve"), Some(0xabcd), "hello there"));
}

#[test]
fn death_messages_are_only_our_own() {
    let mut body = Vec::new();
    write_varint(3, &mut body);
    component("Rider fell from a high place", &mut body);
    assert_eq!(read(from::PLAYER_COMBAT_KILL, &body, &[], 4).unwrap(), None);
    let m = read(from::PLAYER_COMBAT_KILL, &body, &[], 3).unwrap().unwrap();
    assert_eq!((m.kind, m.text.as_str()), (MessageKind::Death, "Rider fell from a high place"));
}
