//! Secure chat signing: the profile key, a chat session and its message chain (`LocalChatSession`,
//! `SignedMessageChain.Encoder`, `PlayerChatMessage.updateSignature`).

use acacia_auth::JavaProfileKey;
use acacia_java_proto::chat::{ChatSessionUpdate, MessageSignature, SIGNATURE_BYTES};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::rand_core::{OsRng, RngCore};
use rsa::signature::{SignatureEncoding, Signer as _};
use sha2::Sha256;

use crate::frame::Error;

/// `PlayerChatMessage`'s signed format version.
const MESSAGE_VERSION: i32 = 1;

/// What one signature covers besides the link: `SignedMessageBody`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Body<'a> {
    pub content: &'a str,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub last_seen: &'a [MessageSignature],
}

/// The bytes SHA256withRSA signs for message `index` of `session`.
pub(crate) fn signed_bytes(sender: u128, session: u128, index: i32, body: &Body) -> Vec<u8> {
    let content = body.content.as_bytes();
    let mut out = Vec::with_capacity(64 + content.len() + body.last_seen.len() * SIGNATURE_BYTES);
    out.extend(MESSAGE_VERSION.to_be_bytes());
    out.extend(sender.to_be_bytes());
    out.extend(session.to_be_bytes());
    out.extend(index.to_be_bytes());
    out.extend(body.salt.to_be_bytes());
    // `Instant.getEpochSecond`: seconds, while the packet carries milliseconds.
    out.extend(body.timestamp_millis.div_euclid(1000).to_be_bytes());
    out.extend((content.len() as i32).to_be_bytes());
    out.extend(content);
    out.extend((body.last_seen.len() as i32).to_be_bytes());
    body.last_seen.iter().for_each(|s| out.extend(s.0));
    out
}

/// A profile key ready to sign with.
pub(crate) struct ChatKey {
    signer: SigningKey<Sha256>,
    profile: JavaProfileKey,
}

impl ChatKey {
    pub(crate) fn new(profile: JavaProfileKey) -> Result<Self, Error> {
        let key = RsaPrivateKey::from_pkcs8_der(&profile.private_key).map_err(|e| Error::Crypto(format!("profile key: {e}")))?;
        Ok(Self { signer: SigningKey::new(key), profile })
    }

    pub(crate) fn profile(&self) -> &JavaProfileKey {
        &self.profile
    }

    fn sign(&self, bytes: &[u8]) -> Option<MessageSignature> {
        self.signer.try_sign(bytes).ok()?.to_vec().try_into().ok().map(MessageSignature)
    }
}

/// `LocalChatSession` with its encoder's chain: a random session id and the next link index.
pub(crate) struct Session {
    pub id: u128,
    sender: u128,
    /// `None` once the chain is exhausted at `i32::MAX`.
    next: Option<i32>,
}

impl Session {
    pub(crate) fn new(sender: u128) -> Self {
        Self { id: random_uuid(), sender, next: Some(0) }
    }

    /// `ServerboundChatSessionUpdatePacket` announcing this session.
    pub(crate) fn update(&self, key: &ChatKey) -> ChatSessionUpdate {
        let p = &key.profile;
        ChatSessionUpdate { session: self.id, expires_at_millis: p.expires_at, public_key: p.public_key.clone(), key_signature: p.signature.clone() }
    }

    /// Signs `body` as the chain's next link (`SignedMessageChain.Encoder.pack`).
    pub(crate) fn sign(&mut self, key: &ChatKey, body: &Body) -> Option<MessageSignature> {
        let index = self.next?;
        self.next = index.checked_add(1);
        key.sign(&signed_bytes(self.sender, self.id, index, body))
    }
}

/// `UUID.randomUUID`: version 4, IETF variant.
fn random_uuid() -> u128 {
    let mut bytes = [0; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    u128::from_be_bytes(bytes)
}

/// `Crypt.SaltSupplier.getLong`.
pub(crate) fn salt() -> i64 {
    OsRng.next_u64() as i64
}

#[cfg(test)]
#[path = "signing_tests.rs"]
mod tests;
