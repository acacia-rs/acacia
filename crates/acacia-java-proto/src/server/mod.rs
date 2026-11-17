//! Server-role codecs: the handshake, status, login and configuration packets a server sends (encode) and receives
//! (decode), plus length-prefixed framing. Every body is a [`Wire`](crate::Wire), so both directions round-trip.
//! Ids live in [`crate::ids`]; layouts were checked with `javap -p -c` on the 26.3 codec classes.

pub mod config;
pub mod frame;
pub mod login;
pub mod status;

use crate::Error;
pub use crate::packets::{Empty, Intention, KeepAlive, KnownPack, LoginCompression, LoginHello, SelectKnownPacks};

/// `ClientIntent`: the handshake's next state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Status,
    Login,
    Transfer,
}

impl Intent {
    pub fn from_id(id: i32) -> Result<Self, Error> {
        match id {
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            3 => Ok(Self::Transfer),
            _ => Err(Error::Invalid("client intent")),
        }
    }

    pub fn id(self) -> i32 {
        match self {
            Self::Status => 1,
            Self::Login => 2,
            Self::Transfer => 3,
        }
    }
}

impl Intention {
    pub fn intent(&self) -> Result<Intent, Error> {
        Intent::from_id(self.intent.0)
    }
}

/// A packet whose body is the rest of the frame (custom payload data).
pub(crate) fn read_rest(input: &mut &[u8]) -> Vec<u8> {
    std::mem::take(input).to_vec()
}

#[cfg(test)]
pub(crate) fn round_trip<T: crate::Wire + PartialEq + std::fmt::Debug>(value: T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.write(&mut bytes);
    let mut input = &bytes[..];
    assert_eq!(T::read(&mut input).unwrap(), value);
    assert!(input.is_empty(), "trailing bytes");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VarInt;

    #[test]
    fn intention_round_trips_and_maps_intents() {
        let bytes = round_trip(Intention { protocol: VarInt(777), host: "localhost".into(), port: 25565, intent: VarInt(2) });
        assert_eq!(bytes[..3], [0x89, 0x06, 9]);
        assert_eq!(&bytes[bytes.len() - 3..], &[0x63, 0xdd, 2]);
        for intent in [Intent::Status, Intent::Login, Intent::Transfer] {
            assert_eq!(Intent::from_id(intent.id()), Ok(intent));
        }
        assert!(Intent::from_id(0).is_err());
    }
}
