//! Chat: what clients send (signed or not, commands, acknowledgements, session keys) and what the server relays.
//! Reads enforce the codecs' caps (`stringUtf8(n)`, `list(n)`) the way the vanilla decoder does.

use super::{OptionalText, read_capped_bytes, read_capped_len, read_capped_string};
use crate::text::Component;
use crate::wire::take;
use crate::{Error, VarInt, Wire, read_varint, write_varint};

// Sizes and caps: `MessageSignature.BYTES`; `fixedBitSet(20)`; `stringUtf8(256)` and `(32767)`;
// `ArgumentSignatures` = `list(8)` of (`stringUtf8(16)`, signature); `LastSeenMessages.Packed` = `list(20)`;
// the public key and its signature as `ProfilePublicKey.Data` caps them.
pub const SIGNATURE_BYTES: usize = 256;
pub const ACKNOWLEDGED_BYTES: usize = 3;
pub const MAX_MESSAGE: usize = 256;
pub const MAX_COMMAND: usize = 32767;
pub const MAX_ARGUMENT_SIGNATURES: usize = 8;
pub const MAX_ARGUMENT_NAME: usize = 16;
pub const MAX_LAST_SEEN: usize = 20;
pub const MAX_PUBLIC_KEY: usize = 512;
pub const MAX_KEY_SIGNATURE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSignature(pub [u8; SIGNATURE_BYTES]);

impl Wire for MessageSignature {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self(take(input, SIGNATURE_BYTES)?.try_into().expect("exact length")))
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.0);
    }
}

impl Wire for Option<MessageSignature> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { MessageSignature::read(input).map(Some) } else { Ok(None) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.is_some().write(out);
        if let Some(signature) = self {
            signature.write(out);
        }
    }
}

/// `MessageSignature.Packed`: a cache index plus one, or 0 and the whole signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedSignature {
    Cached(i32),
    Full(MessageSignature),
}

impl Wire for PackedSignature {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match read_varint(input)? {
            0 => MessageSignature::read(input).map(Self::Full),
            n => Ok(Self::Cached(n - 1)),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Cached(index) => write_varint(index + 1, out),
            Self::Full(signature) => {
                write_varint(0, out);
                signature.write(out);
            }
        }
    }
}

/// `LastSeenMessages.Update`: messages seen since the last packet, the acknowledged set, a checksum (0 skips it).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LastSeenUpdate {
    pub offset: i32,
    pub acknowledged: [u8; ACKNOWLEDGED_BYTES],
    pub checksum: u8,
}

impl Wire for LastSeenUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            offset: read_varint(input)?,
            acknowledged: take(input, ACKNOWLEDGED_BYTES)?.try_into().expect("exact length"),
            checksum: u8::read(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.offset, out);
        out.extend_from_slice(&self.acknowledged);
        self.checksum.write(out);
    }
}

/// `ServerboundChatPacket`; timestamps are the `Instant` as epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedChat {
    pub message: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub signature: Option<MessageSignature>,
    pub last_seen: LastSeenUpdate,
}

impl Wire for SignedChat {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            message: read_capped_string(input, MAX_MESSAGE)?,
            timestamp_millis: i64::read(input)?,
            salt: i64::read(input)?,
            signature: Option::read(input)?,
            last_seen: LastSeenUpdate::read(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.message.write(out);
        self.timestamp_millis.write(out);
        self.salt.write(out);
        self.signature.write(out);
        self.last_seen.write(out);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentSignature {
    pub name: String,
    pub signature: MessageSignature,
}

impl Wire for ArgumentSignature {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { name: read_capped_string(input, MAX_ARGUMENT_NAME)?, signature: MessageSignature::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.name.write(out);
        self.signature.write(out);
    }
}

/// `ServerboundChatCommandSignedPacket`: the command without its slash, then the signatures of its message arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCommandSigned {
    pub command: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub arguments: Vec<ArgumentSignature>,
    pub last_seen: LastSeenUpdate,
}

impl Wire for ChatCommandSigned {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let command = read_capped_string(input, MAX_COMMAND)?;
        let (timestamp_millis, salt) = (i64::read(input)?, i64::read(input)?);
        let arguments = (0..read_capped_len(input, MAX_ARGUMENT_SIGNATURES)?).map(|_| ArgumentSignature::read(input)).collect::<Result<_, _>>()?;
        Ok(Self { command, timestamp_millis, salt, arguments, last_seen: LastSeenUpdate::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.command.write(out);
        self.timestamp_millis.write(out);
        self.salt.write(out);
        self.arguments.write(out);
        self.last_seen.write(out);
    }
}

// Serverbound `ChatAck`: how many messages the client has seen since its last acknowledgement.
crate::packets::packet!(ChatAck { offset: VarInt });

/// `ServerboundChatSessionUpdatePacket`: the session id and the player's public key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSessionUpdate {
    pub session_id: u128,
    pub expires_at_millis: i64,
    pub public_key: Vec<u8>,
    pub key_signature: Vec<u8>,
}

impl Wire for ChatSessionUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            session_id: u128::read(input)?,
            expires_at_millis: i64::read(input)?,
            public_key: read_capped_bytes(input, MAX_PUBLIC_KEY)?,
            key_signature: read_capped_bytes(input, MAX_KEY_SIGNATURE)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.session_id.write(out);
        self.expires_at_millis.write(out);
        write_varint(self.public_key.len() as i32, out);
        out.extend_from_slice(&self.public_key);
        write_varint(self.key_signature.len() as i32, out);
        out.extend_from_slice(&self.key_signature);
    }
}

/// `ChatType.Bound`: a `chat_type` registry holder (written as id + 1, so an inline type can't be sent), the
/// sender's display name, and an optional target name.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatTypeBound {
    pub chat_type: i32,
    pub name: Component,
    pub target: OptionalText,
}

impl Wire for ChatTypeBound {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let chat_type = match read_varint(input)? {
            0 => return Err(Error::Invalid("inline chat type")),
            n => n - 1,
        };
        Ok(Self { chat_type, name: Component::read(input)?, target: OptionalText::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.chat_type + 1, out);
        self.name.write(out);
        self.target.write(out);
    }
}

/// `FilterMask`: pass through 0, fully filtered 1, or partially 2 with the filtered positions as a long array.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FilterMask {
    #[default]
    PassThrough,
    FullyFiltered,
    Partially(Vec<i64>),
}

impl Wire for FilterMask {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match read_varint(input)? {
            0 => Ok(Self::PassThrough),
            1 => Ok(Self::FullyFiltered),
            2 => Vec::read(input).map(Self::Partially),
            _ => Err(Error::Invalid("filter mask type")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::PassThrough => write_varint(0, out),
            Self::FullyFiltered => write_varint(1, out),
            Self::Partially(bits) => {
                write_varint(2, out);
                bits.write(out);
            }
        }
    }
}

/// `SignedMessageBody.Packed`: text, time, salt and the previous messages' signatures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedBody {
    pub content: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub last_seen: Vec<PackedSignature>,
}

impl Wire for SignedBody {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let content = read_capped_string(input, MAX_MESSAGE)?;
        let (timestamp_millis, salt) = (i64::read(input)?, i64::read(input)?);
        let last_seen = (0..read_capped_len(input, MAX_LAST_SEEN)?).map(|_| PackedSignature::read(input)).collect::<Result<_, _>>()?;
        Ok(Self { content, timestamp_millis, salt, last_seen })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.content.write(out);
        self.timestamp_millis.write(out);
        self.salt.write(out);
        self.last_seen.write(out);
    }
}

body!(
    /// `ClientboundPlayerChatPacket`. `global_index` counts every chat line the client has been sent; `index`
    /// this sender's. `signature` is absent for a message the server relays unsigned.
    PlayerChat {
        global_index: VarInt,
        sender: u128,
        index: VarInt,
        signature: Option<MessageSignature>,
        body: SignedBody,
        unsigned_content: OptionalText,
        filter_mask: FilterMask,
        chat_type: ChatTypeBound,
    }
);

body!(DisguisedChat { message: Component, chat_type: ChatTypeBound });
body!(DeleteChat { signature: PackedSignature });
