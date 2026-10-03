//! Decoded field values must match what the JS reference decoded from the same bytes
//! (byte-exact round trips alone can't catch a symmetric misread).

use acacia_proto::packets::*;
use acacia_proto::{Packet, RawPacket};
use bytes::Bytes;
use serde_json::Value;

fn samples<T: Packet>() -> Vec<(T, Value)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packets");
    let hex = std::fs::read_to_string(dir.join(format!("{}.hex", T::NAME))).unwrap();
    let json: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(format!("{}.json", T::NAME))).unwrap(),
    )
    .unwrap();
    hex.lines()
        .zip(json.as_array().unwrap())
        .map(|(line, j)| {
            let bytes: Vec<u8> = (0..line.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&line[i..i + 2], 16).unwrap())
                .collect();
            (
                RawPacket::parse(Bytes::from(bytes))
                    .unwrap()
                    .decode::<T>()
                    .unwrap(),
                j.clone(),
            )
        })
        .collect()
}

/// JS mapper names are snake_case; Rust variants are their PascalCase (`unknown` → `UnknownValue`).
fn variant(v: impl std::fmt::Debug, js: &Value) -> bool {
    let pascal: String = js
        .as_str()
        .unwrap()
        .split('_')
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect();
    let pascal = if pascal == "Unknown" {
        "UnknownValue".to_owned()
    } else {
        pascal
    };
    format!("{v:?}") == pascal
}

fn int(v: &Value) -> i128 {
    match v {
        Value::Number(n) => n.as_i64().unwrap() as i128,
        Value::String(s) => s.parse().unwrap(),
        Value::Array(hl) => {
            ((hl[0].as_i64().unwrap() as i128) << 32) | hl[1].as_i64().unwrap() as i128
        }
        other => panic!("not an int: {other}"),
    }
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap() as f32
}

#[test]
fn start_game_fields() {
    for (p, j) in samples::<StartGame>() {
        assert_eq!(p.entity_id as i128, int(&j["entity_id"]));
        assert_eq!(p.runtime_entity_id as i128, int(&j["runtime_entity_id"]));
        assert_eq!(p.seed as i128, int(&j["seed"]));
        assert!(variant(p.player_gamemode, &j["player_gamemode"]));
        assert_eq!(p.player_position.y, f(&j["player_position"]["y"]));
        assert_eq!(p.level_id, j["level_id"]);
        assert_eq!(p.world_name, j["world_name"]);
        assert_eq!(
            p.block_properties.len(),
            j["block_properties"].as_array().unwrap().len()
        );
    }
}

#[test]
fn move_player_fields() {
    for (p, j) in samples::<MovePlayer>() {
        assert_eq!(p.runtime_id as i128, int(&j["runtime_id"]));
        assert_eq!(p.position.x, f(&j["position"]["x"]));
        assert!(variant(p.mode, &j["mode"]));
        assert_eq!(p.tick as i128, int(&j["tick"]));
        match (&p.teleport, j.get("teleport")) {
            (Some(t), Some(jt)) => assert!(variant(t.cause, &jt["cause"])),
            (None, None) => {}
            other => panic!("teleport mismatch: {other:?}"),
        }
    }
}

#[test]
fn item_registry_entries() {
    for (p, j) in samples::<ItemRegistry>() {
        let js = j["itemstates"].as_array().unwrap();
        assert_eq!(p.itemstates.len(), js.len());
        for (item, ji) in p.itemstates.iter().zip(js) {
            assert_eq!(item.name, ji["name"]);
            assert_eq!(item.runtime_id as i128, int(&ji["runtime_id"]));
            assert!(variant(item.version, &ji["version"]));
        }
    }
}

#[test]
fn text_messages() {
    for (p, j) in samples::<Text>() {
        assert!(variant(p.r#type, &j["type"]));
        let message = match &p.content {
            TextContent::Chat(c) => &c.message,
            TextContent::Whisper(c) => &c.message,
            TextContent::Announcement(c) => &c.message,
            TextContent::Raw(c) => &c.message,
            TextContent::Tip(c) => &c.message,
            TextContent::System(c) => &c.message,
            TextContent::JsonWhisper(c) => &c.message,
            TextContent::Json(c) => &c.message,
            TextContent::JsonAnnouncement(c) => &c.message,
            TextContent::Translation(c) => &c.message,
            TextContent::Popup(c) => &c.message,
            TextContent::JukeboxPopup(c) => &c.message,
            TextContent::Default => panic!("unexpected default"),
        };
        assert_eq!(message, &j["message"]);
        assert_eq!(
            p.filtered_message.as_deref(),
            j.get("filtered_message").and_then(Value::as_str)
        );
    }
}
