//! The server side of a game connection: batching plus the compression and encryption switches a
//! server makes during login. A switch never applies to the batch carrying the packet that announces
//! it (NetworkSettings, ServerToClientHandshake), only to the batches after it.

use acacia_auth::login::{build_server_handshake, client_public_key};
use acacia_proto::packets::{NetworkSettings, ServerToClientHandshake};
use acacia_proto::{codec, encode_packet};
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;

use crate::batch::BatchCodec;
use crate::compression::Algorithm;
use crate::crypto::derive_key;
use crate::Error;

pub struct ServerConnection {
    codec: BatchCodec,
    key: SigningKey,
    pending: Option<Switch>,
}

enum Switch {
    Compression(Algorithm, usize),
    Encryption([u8; 32]),
}

impl ServerConnection {
    /// `key` signs the handshake; a fresh random one per connection is what BDS does.
    pub fn new(key: SigningKey) -> Self {
        Self { codec: BatchCodec::default(), key, pending: None }
    }

    /// The client's packets in a game message, each with its header.
    pub fn decode(&mut self, msg: &[u8]) -> Result<Vec<Bytes>, Error> {
        let mut packets = Vec::new();
        self.codec.decode(msg, &mut packets)?;
        Ok(packets)
    }

    /// One game message, then any switch started since the previous one.
    pub fn encode(&mut self, packets: &[Bytes]) -> Bytes {
        let batch = self.codec.encode(packets.iter().map(|p| &p[..]));
        match self.pending.take() {
            Some(Switch::Compression(alg, threshold)) => self.codec.enable_compression(alg, threshold),
            Some(Switch::Encryption(key)) => self.codec.enable_encryption(key),
            None => {}
        }
        batch
    }

    /// Compresses as `settings` says from the batch after the next [`encode`](Self::encode).
    pub fn start_compression(&mut self, settings: &NetworkSettings) -> Result<(), Error> {
        let alg = Algorithm::from_settings(settings.compression_algorithm)?;
        self.pending = Some(Switch::Compression(alg, settings.compression_threshold.into()));
        Ok(())
    }

    /// The ServerToClientHandshake for a client key; encryption starts after the batch carrying it.
    pub fn start_encryption(&mut self, client_key: &p384::PublicKey) -> Bytes {
        let (token, salt) = build_server_handshake(&self.key);
        self.pending = Some(Switch::Encryption(derive_key(&self.key, client_key, &salt)));
        let mut packet = BytesMut::new();
        encode_packet(&ServerToClientHandshake { token }, &mut packet);
        packet.freeze()
    }
}

/// A Login body split into its protocol version, connection request and the client key in it.
pub fn read_login(body: &[u8]) -> Result<(i32, &[u8], p384::PublicKey), Error> {
    let mut r = body;
    let protocol = codec::read_i32(&mut r)?;
    let len = codec::read_varint(&mut r)? as usize;
    let request = r.get(..len).ok_or_else(|| Error::Login("connection request truncated".into()))?;
    let key = client_public_key(request).map_err(|e| Error::Login(e.to_string()))?;
    Ok((protocol, request, key))
}
