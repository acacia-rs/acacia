//! `ClientboundPlayerInfoUpdatePacket` and `ClientboundPlayerInfoRemovePacket`. Layout from the `Action` codecs
//! (javap): azalea skipped list order and hat, shifting every later entry.

use super::Optional;
use crate::packets::packet;
use crate::server::login::ProfileProperty;
use crate::text::Component;
use crate::types::GameMode;
use crate::wire::{read_len, take};
use crate::{Error, VarInt, Wire};

/// `ClientboundPlayerInfoUpdatePacket.Action`, in declaration order; the wire set is a byte with bit = ordinal.
pub const ADD_PLAYER: u8 = 1;
pub const INITIALIZE_CHAT: u8 = 1 << 1;
pub const UPDATE_GAME_MODE: u8 = 1 << 2;
pub const UPDATE_LISTED: u8 = 1 << 3;
pub const UPDATE_LATENCY: u8 = 1 << 4;
pub const UPDATE_DISPLAY_NAME: u8 = 1 << 5;
pub const UPDATE_LIST_ORDER: u8 = 1 << 6;
pub const UPDATE_HAT: u8 = 1 << 7;

const MAX_KEY_BYTES: usize = 512;
const MAX_SIGNATURE_BYTES: usize = 4096;

/// `RemoteChatSession.Data`: the session id and the player's public key (`ProfilePublicKey.Data`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatSession {
    pub session_id: u128,
    pub expires_at_millis: i64,
    pub public_key: Vec<u8>,
    pub key_signature: Vec<u8>,
}

fn read_bytes(input: &mut &[u8], max: usize, what: &'static str) -> Result<Vec<u8>, Error> {
    let len = read_len(input)?;
    if len > max {
        return Err(Error::Invalid(what));
    }
    Ok(take(input, len)?.to_vec())
}

impl Wire for ChatSession {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            session_id: u128::read(input)?,
            expires_at_millis: i64::read(input)?,
            public_key: read_bytes(input, MAX_KEY_BYTES, "public key length")?,
            key_signature: read_bytes(input, MAX_SIGNATURE_BYTES, "key signature length")?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.session_id.write(out);
        self.expires_at_millis.write(out);
        self.public_key.write(out);
        self.key_signature.write(out);
    }
}

/// One entry; only the fields whose action is in the packet's set are on the wire, the rest stay default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerInfoEntry {
    pub uuid: u128,
    /// `ADD_PLAYER`: the name (at most 16 characters) and the skin properties (at most 16).
    pub name: String,
    pub properties: Vec<ProfileProperty>,
    pub chat_session: Optional<ChatSession>,
    pub game_mode: GameMode,
    pub listed: bool,
    pub latency: i32,
    pub display_name: Optional<Component>,
    pub list_order: i32,
    pub show_hat: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayerInfoUpdate {
    /// Bits of the action constants in this module.
    pub actions: u8,
    pub entries: Vec<PlayerInfoEntry>,
}

impl PlayerInfoEntry {
    fn read(actions: u8, input: &mut &[u8]) -> Result<Self, Error> {
        let mut entry = Self { uuid: u128::read(input)?, ..Self::default() };
        if actions & ADD_PLAYER != 0 {
            entry.name = String::read(input)?;
            entry.properties = Vec::read(input)?;
        }
        if actions & INITIALIZE_CHAT != 0 {
            entry.chat_session = Optional::read(input)?;
        }
        if actions & UPDATE_GAME_MODE != 0 {
            entry.game_mode = GameMode::read(input)?;
        }
        if actions & UPDATE_LISTED != 0 {
            entry.listed = bool::read(input)?;
        }
        if actions & UPDATE_LATENCY != 0 {
            entry.latency = VarInt::read(input)?.0;
        }
        if actions & UPDATE_DISPLAY_NAME != 0 {
            entry.display_name = Optional::read(input)?;
        }
        if actions & UPDATE_LIST_ORDER != 0 {
            entry.list_order = VarInt::read(input)?.0;
        }
        if actions & UPDATE_HAT != 0 {
            entry.show_hat = bool::read(input)?;
        }
        Ok(entry)
    }

    fn write(&self, actions: u8, out: &mut Vec<u8>) {
        self.uuid.write(out);
        if actions & ADD_PLAYER != 0 {
            self.name.write(out);
            self.properties.write(out);
        }
        if actions & INITIALIZE_CHAT != 0 {
            self.chat_session.write(out);
        }
        if actions & UPDATE_GAME_MODE != 0 {
            self.game_mode.write(out);
        }
        if actions & UPDATE_LISTED != 0 {
            self.listed.write(out);
        }
        if actions & UPDATE_LATENCY != 0 {
            VarInt(self.latency).write(out);
        }
        if actions & UPDATE_DISPLAY_NAME != 0 {
            self.display_name.write(out);
        }
        if actions & UPDATE_LIST_ORDER != 0 {
            VarInt(self.list_order).write(out);
        }
        if actions & UPDATE_HAT != 0 {
            self.show_hat.write(out);
        }
    }
}

impl Wire for PlayerInfoUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let actions = u8::read(input)?;
        let count = read_len(input)?;
        let mut entries = Vec::with_capacity(count.min(input.len()));
        for _ in 0..count {
            entries.push(PlayerInfoEntry::read(actions, input)?);
        }
        Ok(Self { actions, entries })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.actions.write(out);
        VarInt(self.entries.len() as i32).write(out);
        self.entries.iter().for_each(|entry| entry.write(self.actions, out));
    }
}

packet!(PlayerInfoRemove { uuids: Vec<u128> });
