//! Strict decoding: reports what the normal decoder accepts leniently (see docs/proto.md, "Strict decoding").

use std::cell::Cell;

use thiserror::Error;

use crate::{DecodeError, Packet, RawPacket, packets};

/// Input the normal decoder accepts although a schema-exact peer would not send it.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Leniency {
    #[error("{ty} has no value {value}")]
    UnknownEnum { ty: &'static str, value: i64 },
    #[error("bool byte is {0}")]
    Bool(u8),
    #[error("string is not UTF-8")]
    Utf8,
    #[error("list ends before its count")]
    TruncatedList,
    #[error("{0} unread bytes in a length-prefixed block")]
    BlockRest(usize),
}

thread_local! {
    static FIRST: Cell<Option<Leniency>> = const { Cell::new(None) };
}

/// Records a leniency for the decode in progress; the first one wins.
#[cold]
pub(crate) fn note(leniency: Leniency) {
    FIRST.with(|first| {
        if first.get().is_none() {
            first.set(Some(leniency));
        }
    });
}

impl RawPacket {
    /// Like [`RawPacket::decode`], but a leniency or bytes left after the body are errors.
    pub fn decode_strict<T: Packet>(&self) -> Result<T, DecodeError> {
        if self.id != T::ID {
            return Err(DecodeError::IdMismatch { expected: T::ID, actual: self.id });
        }
        FIRST.set(None);
        let mut r = &self.body[..];
        let decoded = T::decode(&mut r);
        let noted = FIRST.take();
        let packet = decoded.map_err(|e| e.at(T::NAME))?;
        if let Some(leniency) = noted {
            return Err(DecodeError::Lenient(leniency).at(T::NAME));
        }
        if !r.is_empty() {
            return Err(DecodeError::TrailingBytes(r.len()).at(T::NAME));
        }
        Ok(packet)
    }
}

macro_rules! check_table {
    ($($t:ident),* $(,)?) => {
        /// Strictly decodes `raw` as whatever packet its id names and drops the result.
        pub fn check(raw: &RawPacket) -> Result<(), DecodeError> {
            $(if raw.id == <packets::$t as Packet>::ID { return raw.decode_strict::<packets::$t>().map(drop); })*
            Err(DecodeError::UnknownPacket(raw.id))
        }
    };
}
crate::for_each_packet!(check_table);
