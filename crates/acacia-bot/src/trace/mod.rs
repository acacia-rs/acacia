//! Movement traces: the packets that drive physics plus every `PlayerAuthInput`, recorded live
//! (see [`crate::BotConfig::record`]) and replayed offline through the same movement code
//! ([`replay`]), so physics changes can be checked against a session in milliseconds.
//!
//! File: `BTRC` + version byte, then events: kind `u8`, then for a packet `id u32, len u32, body`,
//! for an input `len u32, body`, for a start `feet [f32; 3], yaw f32, pitch f32`, for a mark
//! `len u32, utf8`, for equipment five `i32` (depth strider, soul speed, swift sneak, leather boots, elytra),
//! for a ride a tag `u8` and its `f32`s ([`Ride`]), for a glide boost nothing, for solids a count `u32` and
//! per box min and max as six `f32`, for a riptide the level `u8`.
//! Integers and floats are little-endian.

mod blocks;
mod replay;
mod report;
mod ride;

pub use replay::replay;
pub use report::{CorrectionDiff, Report, TickDiff, CORRECTION_TOLERANCE};
pub use ride::Ride;

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use acacia_client::proto::packets::{PlayerAuthInput, SetEntityData};
use acacia_client::proto::types::{MetadataDictionaryItemValue, MetadataFlags1};
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::{Aabb, Equipment, Vec3};
use bytes::{Bytes, BytesMut};

/// Recorded for analysis only, when about the recording player: its actor flags (the server's sprint state).
pub const ANALYSIS_PACKETS: &[u32] = &[SetEntityData::ID];

const MAGIC: &[u8; 5] = b"BTRC\x01";
const PACKET: u8 = 0;
const INPUT: u8 = 1;
const START: u8 = 2;
const MARK: u8 = 3;
const EQUIPMENT: u8 = 4;
const RIDE: u8 = 5;
const GLIDE_BOOST: u8 = 6;
const SOLIDS: u8 = 7;
const RIPTIDE: u8 = 8;

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
    /// A driven vehicle's simulation started, or the player left a seat.
    Ride(Ride),
    /// The client used a firework rocket while gliding and predicts its boost from the next input.
    GlideBoost,
    /// The solid entity boxes (boats) the simulation collides with from the next input on, until the next.
    Solids(Vec<Aabb>),
    /// The client released a Riptide trident of this level and predicts the launch from the next input.
    Riptide(u8),
}

/// Whether an [`ANALYSIS_PACKETS`] packet concerns this runtime id.
pub fn about(packet: &RawPacket, runtime_id: u64) -> bool {
    packet.is::<SetEntityData>() && crate::state::leading_entity(packet) == Some(runtime_id)
}

/// The server's actor flags, when this `SetEntityData` sets them.
pub fn actor_flags(packet: &SetEntityData) -> Option<MetadataFlags1> {
    packet.metadata.iter().find_map(|m| match &m.value {
        MetadataDictionaryItemValue::Flags(f) => Some(*f),
        _ => None,
    })
}

pub struct Recorder {
    w: BufWriter<File>,
    /// The last [`Event::Solids`] written.
    solids: Vec<Aabb>,
}

impl Recorder {
    pub fn create(path: &Path) -> io::Result<Self> {
        let mut w = BufWriter::new(File::create(path)?);
        w.write_all(MAGIC)?;
        Ok(Self { w, solids: Vec::new() })
    }

    /// Writes [`Event::Solids`] when `boxes` differ from the last written.
    pub fn solids(&mut self, boxes: &[Aabb]) {
        if self.solids != boxes {
            self.solids = boxes.to_vec();
            self.write(&Event::Solids(self.solids.clone()));
        }
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
        let _ = self.w.flush();
    }

    fn try_write(&mut self, event: &Event) -> io::Result<()> {
        let w = &mut self.w;
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
                for level in [e.depth_strider, e.soul_speed, e.swift_sneak, i32::from(e.leather_boots), elytra_code(e)] {
                    w.write_all(&level.to_le_bytes())?;
                }
                Ok(())
            }
            Event::Ride(ride) => {
                let (tag, values) = ride.tag_and_values();
                w.write_all(&[RIDE, tag])?;
                for v in values {
                    w.write_all(&v.to_le_bytes())?;
                }
                Ok(())
            }
            Event::GlideBoost => w.write_all(&[GLIDE_BOOST]),
            Event::Solids(boxes) => {
                w.write_all(&[SOLIDS])?;
                w.write_all(&(boxes.len() as u32).to_le_bytes())?;
                for b in boxes {
                    for v in b.min.iter().chain(&b.max) {
                        w.write_all(&v.to_le_bytes())?;
                    }
                }
                Ok(())
            }
            Event::Riptide(level) => w.write_all(&[RIPTIDE, *level]),
        }
    }
}

/// The equipment event's elytra field: 0 none, 1 worn with unknown wear (older traces), 2 + glides left.
fn elytra_code(e: &Equipment) -> i32 {
    match (e.elytra, e.elytra_uses) {
        (false, _) => 0,
        (true, None) => 1,
        (true, Some(n)) => 2 + n.min(i32::MAX as u32 - 2) as i32,
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
            Event::Equipment(Equipment {
                depth_strider: v[0],
                soul_speed: v[1],
                swift_sneak: v[2],
                leather_boots: v[3] != 0,
                elytra: v[4] != 0,
                elytra_uses: (v[4] >= 2).then(|| (v[4] - 2) as u32),
            })
        }
        RIDE => Event::Ride(Ride::read(r)?),
        GLIDE_BOOST => Event::GlideBoost,
        SOLIDS => {
            let n = read_u32(r)?;
            let mut boxes = Vec::with_capacity(n.min(1024) as usize);
            for _ in 0..n {
                let mut v = [0.0; 6];
                for x in &mut v {
                    *x = f32::from_bits(read_u32(r)?);
                }
                boxes.push(Aabb { min: [v[0], v[1], v[2]], max: [v[3], v[4], v[5]] });
            }
            Event::Solids(boxes)
        }
        RIPTIDE => {
            let mut level = [0];
            r.read_exact(&mut level)?;
            Event::Riptide(level[0])
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
mod tests;
