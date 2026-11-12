//! Chat packets: serverbound messages, commands, acknowledgements and the chat session; the signatures clientbound
//! player chat carries (`ServerboundChat*Packet`, `ClientboundPlayerChatPacket`, `MessageSignature`).

use crate::packets::packet;
use crate::wire::{optional, take};
use crate::{Error, VarInt, Wire, read_varint, write_varint};

pub const SIGNATURE_BYTES: usize = 256;
/// `LastSeenMessages.LAST_SEEN_MESSAGES_MAX_LENGTH`: the acknowledgement window.
pub const LAST_SEEN: usize = 20;
/// `ByteBufCodecs.fixedBitSet(20)`.
const ACKNOWLEDGED_BYTES: usize = LAST_SEEN.div_ceil(8);

/// An RSA-2048 SHA256withRSA signature.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MessageSignature(pub [u8; SIGNATURE_BYTES]);

impl Default for MessageSignature {
    fn default() -> Self {
        Self([0; SIGNATURE_BYTES])
    }
}

impl std::fmt::Debug for MessageSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageSignature({:02x?}..)", &self.0[..4])
    }
}

impl Wire for MessageSignature {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self(take(input, SIGNATURE_BYTES)?.try_into().expect("exact length")))
    }
    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.0);
    }
}
optional!(MessageSignature);

/// `MessageSignature.Packed`: an index into the receiver's `MessageSignatureCache`, or the whole signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackedSignature {
    Cached(usize),
    Full(MessageSignature),
}

impl Wire for PackedSignature {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(match read_varint(input)? {
            0 => Self::Full(MessageSignature::read(input)?),
            id => Self::Cached(usize::try_from(id - 1).map_err(|_| Error::NegativeLength(id))?),
        })
    }
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Full(signature) => {
                write_varint(0, out);
                signature.write(out);
            }
            Self::Cached(id) => write_varint(*id as i32 + 1, out),
        }
    }
}

/// `LastSeenMessages.Update`. The default acknowledges nothing and has checksum 0, `IGNORE_CHECKSUM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LastSeenUpdate {
    pub offset: i32,
    /// Bit `i` acknowledges window slot `i`, oldest first.
    pub acknowledged: u32,
    pub checksum: u8,
}

impl Wire for LastSeenUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let offset = read_varint(input)?;
        let mut bits = [0; 4];
        bits[..ACKNOWLEDGED_BYTES].copy_from_slice(take(input, ACKNOWLEDGED_BYTES)?);
        Ok(Self { offset, acknowledged: u32::from_le_bytes(bits), checksum: u8::read(input)? })
    }
    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.offset, out);
        // `BitSet.toByteArray` is little-endian, padded to the fixed size.
        out.extend_from_slice(&self.acknowledged.to_le_bytes()[..ACKNOWLEDGED_BYTES]);
        out.push(self.checksum);
    }
}

packet!(Chat { message: String, timestamp_millis: i64, salt: i64, signature: Option<MessageSignature>, last_seen: LastSeenUpdate });
packet!(ArgumentSignature { name: String, signature: MessageSignature });
packet!(SignedChatCommand {
    command: String,
    timestamp_millis: i64,
    salt: i64,
    arguments: Vec<ArgumentSignature>,
    last_seen: LastSeenUpdate,
});
packet!(ChatAck { offset: VarInt });
// `RemoteChatSession.Data`: the session id and `ProfilePublicKey.Data` (expiry, SPKI DER key, Mojang's signature).
packet!(ChatSessionUpdate { session: u128, expires_at_millis: i64, public_key: Vec<u8>, key_signature: Vec<u8> });

/// The leading fields of `ClientboundPlayerChatPacket`, up to its body's last-seen list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerChatSignatures {
    pub global_index: i32,
    pub sender: u128,
    pub index: i32,
    pub signature: Option<MessageSignature>,
    pub content: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub last_seen: Vec<PackedSignature>,
}

impl PlayerChatSignatures {
    pub fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            global_index: read_varint(input)?,
            sender: u128::read(input)?,
            index: read_varint(input)?,
            signature: Option::read(input)?,
            content: String::read(input)?,
            timestamp_millis: i64::read(input)?,
            salt: i64::read(input)?,
            last_seen: Vec::read(input)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_seen_update_is_offset_three_bitset_bytes_and_checksum() {
        let update = LastSeenUpdate { offset: 300, acknowledged: 1 | 1 << 9 | 1 << 19, checksum: 0xd5 };
        let mut out = Vec::new();
        update.write(&mut out);
        assert_eq!(out, [0xac, 0x02, 0x01, 0x02, 0x08, 0xd5]);
        assert_eq!(LastSeenUpdate::read(&mut &out[..]), Ok(update));
    }

    #[test]
    fn unsigned_chat_keeps_its_old_bytes() {
        let chat = Chat { message: "hi".into(), timestamp_millis: 1, salt: 2, signature: None, last_seen: LastSeenUpdate::default() };
        let mut out = Vec::new();
        chat.write(&mut out);
        let mut expected = vec![2, b'h', b'i', 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2];
        expected.extend([0, 0, 0, 0, 0, 0]);
        assert_eq!(out, expected);
    }

    #[test]
    fn packed_signatures_offset_cache_ids_by_one() {
        let mut out = Vec::new();
        PackedSignature::Cached(4).write(&mut out);
        assert_eq!(out, [5]);
        assert_eq!(PackedSignature::read(&mut &out[..]), Ok(PackedSignature::Cached(4)));
    }
}
