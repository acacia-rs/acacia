//! Movement traces: the packets that drive physics plus every `PlayerAuthInput`, recorded live
//! (see [`crate::BotConfig::record`]) and replayed offline through the same movement code
//! ([`replay`]), so physics changes can be checked against a session in milliseconds.
//!
//! File: `BTRC` + version byte, then events: kind `u8`, then for a packet `id u32, len u32, body`,
//! for an input `len u32, body`, for a start `feet [f32; 3], yaw f32, pitch f32`, for a mark
//! `len u32, utf8`, for equipment five `i32` (depth strider, soul speed, swift sneak, leather boots, elytra).
//! Integers and floats are little-endian.

mod replay;

pub use replay::{replay, CorrectionDiff, Report, TickDiff, CORRECTION_TOLERANCE};

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use acacia_client::proto::packets::{PlayerAuthInput, SetEntityData};
use acacia_client::proto::types::{MetadataDictionaryItemValue, MetadataFlags1};
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::{Equipment, Vec3};
use bytes::{Bytes, BytesMut};

/// Recorded for analysis only, when about the recording player: its actor flags (the server's sprint state).
pub const ANALYSIS_PACKETS: &[u32] = &[SetEntityData::ID];

const MAGIC: &[u8; 5] = b"BTRC\x01";
const PACKET: u8 = 0;
const INPUT: u8 = 1;
const START: u8 = 2;
const MARK: u8 = 3;
const EQUIPMENT: u8 = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A packet from the server, in arrival order relative to the ticks.
    Packet(RawPacket),
    /// The encoded body of a `PlayerAuthInput` sent to the server (one per tick).
    Input(Bytes),
    /// Movement started at these feet.
    Start { feet: Vec3, yaw: f32, pitch: f32 },
    /// A label, e.g. the drill that starts here.
    Mark(String),
    /// The worn armour from here on (the client predicts some changes the server never sends).
    Equipment(Equipment),
}

/// Whether an [`ANALYSIS_PACKETS`] packet concerns this runtime id.
pub fn about(packet: &RawPacket, runtime_id: u64) -> bool {
    packet.decode::<SetEntityData>().is_ok_and(|p| p.runtime_entity_id == runtime_id)
}

/// The server's actor flags, when this `SetEntityData` sets them.
pub fn actor_flags(packet: &SetEntityData) -> Option<MetadataFlags1> {
    packet.metadata.iter().find_map(|m| match &m.value {
        MetadataDictionaryItemValue::Flags(f) => Some(*f),
        _ => None,
    })
}

pub struct Recorder(BufWriter<File>);

impl Recorder {
    pub fn create(path: &Path) -> io::Result<Self> {
        let mut w = BufWriter::new(File::create(path)?);
        w.write_all(MAGIC)?;
        Ok(Self(w))
    }

    pub fn write(&mut self, event: &Event) {
        if let Err(e) = self.try_write(event) {
            tracing::warn!(error = %e, "trace write failed");
        }
    }

    pub fn input(&mut self, input: &PlayerAuthInput) {
        let mut body = BytesMut::new();
        input.encode(&mut body);
        self.write(&Event::Input(body.freeze()));
        // Once a tick, so a killed process still leaves a usable trace.
        let _ = self.0.flush();
    }

    fn try_write(&mut self, event: &Event) -> io::Result<()> {
        let w = &mut self.0;
        match event {
            Event::Packet(p) => {
                w.write_all(&[PACKET])?;
                w.write_all(&p.id.to_le_bytes())?;
                write_bytes(w, &p.body)
            }
            Event::Input(body) => {
                w.write_all(&[INPUT])?;
                write_bytes(w, body)
            }
            Event::Start { feet, yaw, pitch } => {
                w.write_all(&[START])?;
                for v in [feet[0], feet[1], feet[2], *yaw, *pitch] {
                    w.write_all(&v.to_le_bytes())?;
                }
                Ok(())
            }
            Event::Mark(label) => {
                w.write_all(&[MARK])?;
                write_bytes(w, label.as_bytes())
            }
            Event::Equipment(e) => {
                w.write_all(&[EQUIPMENT])?;
                for level in [e.depth_strider, e.soul_speed, e.swift_sneak, i32::from(e.leather_boots), i32::from(e.elytra)] {
                    w.write_all(&level.to_le_bytes())?;
                }
                Ok(())
            }
        }
    }
}

fn write_bytes(w: &mut impl Write, b: &[u8]) -> io::Result<()> {
    w.write_all(&(b.len() as u32).to_le_bytes())?;
    w.write_all(b)
}

/// Reads a whole trace (gzipped when the name ends in `.gz`); a truncated final event (from a killed recorder)
/// is dropped.
pub fn read(path: &Path) -> io::Result<Vec<Event>> {
    let file = File::open(path)?;
    let gzipped = path.extension().is_some_and(|e| e == "gz");
    let mut r: BufReader<Box<dyn Read>> =
        BufReader::new(if gzipped { Box::new(flate2::read::GzDecoder::new(file)) } else { Box::new(file) });
    let mut magic = [0; 5];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a movement trace"));
    }
    let mut events = Vec::new();
    loop {
        let mut kind = [0];
        if r.read(&mut kind)? == 0 {
            return Ok(events);
        }
        match read_event(&mut r, kind[0]) {
            Ok(e) => events.push(e),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(events),
            Err(e) => return Err(e),
        }
    }
}

fn read_event(r: &mut impl Read, kind: u8) -> io::Result<Event> {
    Ok(match kind {
        PACKET => {
            let id = read_u32(r)?;
            Event::Packet(RawPacket { id, sender_subclient: 0, target_subclient: 0, body: read_bytes(r)? })
        }
        INPUT => Event::Input(read_bytes(r)?),
        START => {
            let mut v = [0.0; 5];
            for x in &mut v {
                *x = f32::from_bits(read_u32(r)?);
            }
            Event::Start { feet: [v[0], v[1], v[2]], yaw: v[3], pitch: v[4] }
        }
        MARK => Event::Mark(String::from_utf8_lossy(&read_bytes(r)?).into_owned()),
        EQUIPMENT => {
            let mut v = [0; 5];
            for x in &mut v {
                *x = read_u32(r)? as i32;
            }
            Event::Equipment(Equipment { depth_strider: v[0], soul_speed: v[1], swift_sneak: v[2], leather_boots: v[3] != 0, elytra: v[4] != 0 })
        }
        k => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unknown trace event {k}"))),
    })
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_bytes(r: &mut impl Read) -> io::Result<Bytes> {
    let mut b = vec![0; read_u32(r)? as usize];
    r.read_exact(&mut b)?;
    Ok(b.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_own_actor_flags() {
        // BDS 1.26 to a vanilla client (runtime id 1): flags only, not sprinting.
        let body = Bytes::from_static(&[1, 1, 0, 7, 7, 0x80, 0x80, 0xc0, 0x80, 0x80, 0x82, 0x80, 3, 0, 0, 0]);
        let packet = RawPacket { id: SetEntityData::ID, sender_subclient: 0, target_subclient: 0, body };
        assert!(about(&packet, 1) && !about(&packet, 2));
        let flags = actor_flags(&packet.decode().unwrap()).unwrap();
        assert_eq!(flags.0 & MetadataFlags1::SPRINTING.0, 0);
    }

    #[test]
    fn events_round_trip() {
        let path = std::env::temp_dir().join(format!("btrc-{}.bin", std::process::id()));
        let events = vec![
            Event::Mark("drill".into()),
            Event::Start { feet: [1.5, -60.0, 2.5], yaw: -90.0, pitch: 0.0 },
            Event::Packet(RawPacket { id: 7, sender_subclient: 0, target_subclient: 0, body: Bytes::from_static(b"abc") }),
            Event::Input(Bytes::from_static(b"xyz")),
        ];
        let mut rec = Recorder::create(&path).unwrap();
        events.iter().for_each(|e| rec.write(e));
        drop(rec);
        assert_eq!(read(&path).unwrap(), events);
        std::fs::remove_file(path).unwrap();
    }
}
