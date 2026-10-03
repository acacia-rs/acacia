//! Minecraft: Bedrock Edition packet definitions, generated from minecraft-data (see docs/proto.md).
//!
//! Decoding is on demand: [`RawPacket::parse`] reads only the header, and
//! [`RawPacket::decode`] parses the body into a typed packet when asked.

use bytes::{Bytes, BytesMut};

pub mod codec;
mod error;
mod generated;
pub mod manual;
pub mod nbt;

pub use error::DecodeError;
pub use generated::{GAME_VERSION, PROTOCOL_VERSION, packets, types};

macro_rules! name_table {
    ($($t:ident),* $(,)?) => {
        /// The packet struct's name for an ID (`PlayerAuthInput`), as capture tools label packets.
        pub fn packet_name(id: u32) -> Option<&'static str> {
            $(if id == <packets::$t as Packet>::ID { return Some(stringify!($t)); })*
            None
        }
    };
}
crate::for_each_packet!(name_table);

pub trait Packet: Sized {
    const ID: u32;
    const NAME: &'static str;
    /// Writes the body only, without the header.
    fn encode(&self, w: &mut BytesMut);
    /// Reads the body only. Trailing bytes are ignored.
    fn decode(r: &mut &[u8]) -> Result<Self, DecodeError>;
}

/// One de-batched packet with its header split off and its body left undecoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub id: u32,
    pub sender_subclient: u8,
    pub target_subclient: u8,
    pub body: Bytes,
}

const ID_MASK: u32 = 0x3ff;

impl RawPacket {
    /// Header is a varuint32: id (10 bits) | sender << 10 (2 bits) | target << 12 (2 bits).
    pub fn parse(buf: Bytes) -> Result<Self, DecodeError> {
        let mut r = &buf[..];
        let header = codec::read_varint(&mut r)?;
        let header_len = buf.len() - r.len();
        Ok(RawPacket {
            id: header & ID_MASK,
            sender_subclient: ((header >> 10) & 3) as u8,
            target_subclient: ((header >> 12) & 3) as u8,
            body: buf.slice(header_len..),
        })
    }

    pub fn decode<T: Packet>(&self) -> Result<T, DecodeError> {
        if self.id != T::ID {
            return Err(DecodeError::IdMismatch {
                expected: T::ID,
                actual: self.id,
            });
        }
        T::decode(&mut &self.body[..]).map_err(|e| e.at(T::NAME))
    }

    pub fn is<T: Packet>(&self) -> bool {
        self.id == T::ID
    }
}

/// Writes the header (both subclients 0) followed by the body.
pub fn encode_packet<T: Packet>(p: &T, out: &mut BytesMut) {
    codec::write_varint(out, T::ID & ID_MASK);
    p.encode(out);
}
