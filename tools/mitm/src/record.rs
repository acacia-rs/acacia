//! The capture: one JSON object per line, `t` = ms since the proxy started. Packets: `dir` (`C>S` =
//! from the game), `id`, `len`, `name` (acacia-proto's struct name), `raw` (hex body) and
//! `proto_error` when acacia-proto can't parse a body (keys print sorted, so `dir` stays first for
//! line-prefix parsers like mitm_trace). Login has a structural `summary` instead of
//! `raw` (login.rs). Events: `login`, `spawned`, `closed`.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use acacia_proto::{packets, Packet, RawPacket};
use serde_json::{json, Value};

/// Bodies above this are not test-decoded (chunks, registries, command trees).
const MAX_DECODED: usize = 16 << 10;

/// Client-data claims that make up a skin. Every other player sees them, so unlike the device ids
/// next to them they hold nothing secret.
const SKIN_CLAIMS: &[&str] = &[
    "AnimatedImageData", "ArmSize", "CapeData", "CapeId", "CapeImageHeight", "CapeImageWidth", "CapeOnClassicSkin",
    "OverrideSkin", "PersonaPieces", "PersonaSkin", "PieceTintColors", "PremiumSkin", "SkinAnimationData", "SkinColor",
    "SkinData", "SkinGeometryData", "SkinGeometryDataEngineVersion", "SkinId", "SkinImageHeight", "SkinImageWidth",
    "SkinResourcePatch", "TrustedSkin",
];

pub struct Recorder {
    dir: PathBuf,
    path: PathBuf,
    out: File,
    start: Instant,
}

impl Recorder {
    pub fn create(dir: &Path, name: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.jsonl"));
        Ok(Self { dir: dir.to_owned(), out: File::create(&path)?, path, start: Instant::now() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&mut self, mut entry: Value) {
        entry["t"] = json!(self.start.elapsed().as_micros() as f64 / 1000.0);
        let mut line = entry.to_string();
        line.push('\n');
        if let Err(e) = self.out.write_all(line.as_bytes()) {
            eprintln!("capture write failed: {e}");
        }
    }

    pub fn packet(&mut self, from_game: bool, raw: &RawPacket) {
        let mut entry = json!({ "dir": dir(from_game), "id": raw.id, "len": raw.body.len(), "raw": hex::encode(&raw.body) });
        if let Some(name) = acacia_proto::packet_name(raw.id) {
            entry["name"] = json!(name);
        }
        if raw.body.len() <= MAX_DECODED
            && let Some(e) = decode_error(raw)
        {
            entry["proto_error"] = json!(e);
        }
        self.write(entry);
    }

    pub fn login(&mut self, len: usize, summary: Value) {
        self.write(json!({ "dir": dir(true), "id": packets::Login::ID, "len": len, "name": "Login", "summary": summary }));
    }

    /// Writes the skin claims to `<dir>/skins/<SkinId>.json` for the bots' skin pool (acacia-auth/assets/skins).
    pub fn save_skin(&self, client_data: &Value) {
        let skin: serde_json::Map<String, Value> = SKIN_CLAIMS.iter().map(|&k| (k.to_owned(), client_data[k].clone())).collect();
        let id: String = client_data["SkinId"].as_str().unwrap_or("unknown").chars().map(|c| if "/\\:.".contains(c) { '_' } else { c }).collect();
        let path = self.dir.join("skins").join(format!("{id}.json"));
        let mut out = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(b" "));
        serde::Serialize::serialize(&skin, &mut ser).expect("json values serialize");
        let result = std::fs::create_dir_all(path.parent().expect("has parent")).and_then(|()| std::fs::write(&path, &out));
        println!("skin {id} saved ({} bytes): {result:?}", out.len());
    }
}

fn dir(from_game: bool) -> &'static str {
    if from_game { "C>S" } else { "S>C" }
}

macro_rules! by_id {
    ($($t:ident),* $(,)?) => {
        fn decode_error(raw: &RawPacket) -> Option<String> {
            $(if raw.id == <packets::$t as Packet>::ID { return raw.decode::<packets::$t>().err().map(|e| e.to_string()); })*
            None
        }
    };
}
acacia_proto::for_each_packet!(by_id);
