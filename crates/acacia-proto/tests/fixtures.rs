//! Differential test: bytes produced by the JS reference (tests/fixtures/gen.mjs) must decode
//! and re-encode byte-for-byte.

use std::path::{Path, PathBuf};

use acacia_proto::{Packet, RawPacket, encode_packet, packets};
use bytes::{Bytes, BytesMut};

/// Packets a client needs; these must have fixtures and pass.
const CLIENT_PACKETS: &[&str] = &[
    "login",
    "play_status",
    "server_to_client_handshake",
    "client_to_server_handshake",
    "disconnect",
    "resource_packs_info",
    "resource_pack_stack",
    "resource_pack_client_response",
    "network_settings",
    "request_network_settings",
    "start_game",
    "text",
    "set_time",
    "set_score",
    "respawn",
    "inventory_content",
    "inventory_slot",
    "item_registry",
    "command_request",
    "interact",
    "container_close",
    "item_stack_request",
    "player_auth_input",
    "move_player",
    "level_chunk",
    "network_chunk_publisher_update",
    "request_chunk_radius",
    "chunk_radius_update",
    "set_local_player_as_initialized",
    "tick_sync",
    "available_commands",
    "crafting_data",
    "add_entity",
    "add_player",
];

pub fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packets")
}

fn roundtrip<T: Packet + std::fmt::Debug + PartialEq>(bytes: &[u8]) -> Result<(), String> {
    let raw =
        RawPacket::parse(Bytes::copy_from_slice(bytes)).map_err(|e| format!("header: {e}"))?;
    let pkt: T = raw.decode().map_err(|e| format!("decode: {e}"))?;
    let mut out = BytesMut::new();
    encode_packet(&pkt, &mut out);
    if out[..] != *bytes {
        let at = out
            .iter()
            .zip(bytes)
            .position(|(a, b)| a != b)
            .unwrap_or(out.len().min(bytes.len()));
        return Err(format!(
            "re-encode differs at byte {at} (len {} vs {})",
            out.len(),
            bytes.len()
        ));
    }
    let again: T = RawPacket::parse(out.freeze()).unwrap().decode().unwrap();
    if again != pkt {
        return Err("decode(encode(x)) != x".into());
    }
    Ok(())
}

macro_rules! dispatch {
    ($($t:ident),*) => {
        fn check(name: &str, bytes: &[u8]) -> Option<Result<(), String>> {
            $(if name == <packets::$t as Packet>::NAME { return Some(roundtrip::<packets::$t>(bytes)); })*
            None
        }
    };
}
acacia_proto::for_each_packet!(dispatch);

fn check_file(name: &str) -> Result<usize, String> {
    let text = std::fs::read_to_string(fixture_dir().join(format!("{name}.hex")))
        .map_err(|e| format!("{e}"))?;
    let mut n = 0;
    for (i, line) in text.lines().filter(|l| !l.is_empty()).enumerate() {
        check(name, &hex(line))
            .ok_or("no Rust packet type with this name")?
            .map_err(|e| format!("sample {i}: {e}"))?;
        n += 1;
    }
    Ok(n)
}

#[test]
fn all_fixtures_roundtrip_byte_exact() {
    let mut failures = Vec::new();
    let mut packets = 0;
    for entry in std::fs::read_dir(fixture_dir()).expect("fixtures dir") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "hex") {
            let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
            match check_file(&name) {
                Ok(_) => packets += 1,
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} packets failed:\n{}",
        failures.len(),
        packets + failures.len(),
        failures.join("\n")
    );
    assert!(packets > 200, "only {packets} fixtures found");
}

#[test]
fn client_packets_have_passing_fixtures() {
    for name in CLIENT_PACKETS {
        let n = check_file(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(n > 0, "{name}: no samples");
    }
}
