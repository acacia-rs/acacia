//! The capture: one JSON object per line, `t` = ms since the proxy started, `session` = which
//! connection of a game it belongs to (counted from 0; a followed transfer starts a new one).
//! Packets: `dir` (`C>S` = from the game), `id`, `len`, `name` (acacia-proto's struct name), `raw`
//! (hex body) and `proto_error` when acacia-proto can't parse a body (keys print sorted, so `dir`
//! stays first for line-prefix parsers like mitm_trace). Login has a structural `summary` instead
//! of `raw` (login.rs). Events: `connected`, `login`, `spawned`, `transfer` (`to`: where the game was
//! sent before the proxy pointed it at itself), `closed`.
//!
//! Packet lines are what each side sent. Where interceptors or an injector made the wire differ,
//! events say how: `dropped` (`to`, `id`, `name`: the packet line before it did not go on) and
//! `sent` (`to` game|server, `why` replace|inject, then `id`, `len`, `name`, `raw`). They carry no
//! `dir`, so readers of packet lines skip them.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use acacia_proto::{packets, Packet, RawPacket};
use bytes::Bytes;
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

/// One capture file shared by every player of a proxy.
pub(crate) type SharedRecorder = Arc<Mutex<Recorder>>;

pub(crate) fn lock(rec: &Mutex<Recorder>) -> MutexGuard<'_, Recorder> {
    rec.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

    /// A `<capture>.datagrams.tsv` next to the capture, on the same clock.
    pub fn datagram_log(&self) -> std::io::Result<DatagramLog> {
        let path = self.path.with_extension("datagrams.tsv");
        let mut out = File::create(&path)?;
        out.write_all(b"t_ms\tdir\tpeer\tfirst_byte\tlen\n")?;
        println!("datagram timings to {}", path.display());
        Ok(DatagramLog { out, start: self.start })
    }

    pub fn write(&mut self, mut entry: Value) {
        entry["t"] = json!(self.start.elapsed().as_micros() as f64 / 1000.0);
        let mut line = entry.to_string();
        line.push('\n');
        if let Err(e) = self.out.write_all(line.as_bytes()) {
            eprintln!("capture write failed: {e}");
        }
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

/// One session's lines in the shared capture.
#[derive(Clone)]
pub(crate) struct SessionLog {
    rec: SharedRecorder,
    session: u32,
}

impl SessionLog {
    pub fn new(rec: SharedRecorder, session: u32) -> Self {
        Self { rec, session }
    }

    pub fn write(&self, mut entry: Value) {
        entry["session"] = json!(self.session);
        lock(&self.rec).write(entry);
    }

    /// What the interceptors made of the packet just logged, unless they forwarded it as it was.
    pub fn outcome(&self, from_game: bool, raw: &RawPacket, packet: &Bytes, out: &[Bytes]) {
        if out == std::slice::from_ref(packet) {
            return;
        }
        if !out.contains(packet) {
            self.write(named(json!({ "event": "dropped", "to": side(!from_game), "id": raw.id }), raw.id));
        }
        out.iter().filter(|p| *p != packet).for_each(|p| self.sent(!from_game, "replace", p));
    }

    /// A packet (header included) the proxy put on the wire that neither side sent.
    pub fn sent(&self, to_game: bool, why: &str, packet: &Bytes) {
        let Ok(raw) = RawPacket::parse(packet.clone()) else { return };
        let entry = json!({ "event": "sent", "to": side(to_game), "why": why, "id": raw.id, "len": raw.body.len(), "raw": hex::encode(&raw.body) });
        self.write(named(entry, raw.id));
    }

    pub fn packet(&self, from_game: bool, raw: &RawPacket) {
        let mut entry = named(json!({ "dir": dir(from_game), "id": raw.id, "len": raw.body.len(), "raw": hex::encode(&raw.body) }), raw.id);
        if raw.body.len() <= MAX_DECODED
            && let Some(e) = decode_error(raw)
        {
            entry["proto_error"] = json!(e);
        }
        self.write(entry);
    }

    pub fn login(&self, len: usize, summary: Value) {
        self.write(json!({ "dir": dir(true), "id": packets::Login::ID, "len": len, "name": "Login", "summary": summary }));
    }

    pub fn save_skin(&self, client_data: &Value) {
        lock(&self.rec).save_skin(client_data);
    }
}

/// Every game-side RakNet datagram, unbuffered so a Ctrl+C keeps it: input for `capdiff pacing`.
pub struct DatagramLog {
    out: File,
    start: Instant,
}

impl DatagramLog {
    pub fn write(&mut self, from_game: bool, peer: std::net::SocketAddr, datagram: &[u8]) {
        let t = self.start.elapsed().as_micros() as f64 / 1000.0;
        let line = format!("{t:.3}\t{}\t{peer}\t{:#04x}\t{}\n", dir(from_game), datagram.first().copied().unwrap_or(0), datagram.len());
        if let Err(e) = self.out.write_all(line.as_bytes()) {
            eprintln!("datagram log write failed: {e}");
        }
    }
}

fn dir(from_game: bool) -> &'static str {
    if from_game { "C>S" } else { "S>C" }
}

fn side(game: bool) -> &'static str {
    if game { "game" } else { "server" }
}

/// `entry` with acacia-proto's `name` for the id, if it has one.
fn named(mut entry: Value, id: u32) -> Value {
    if let Some(name) = acacia_proto::packet_name(id) {
        entry["name"] = json!(name);
    }
    entry
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
