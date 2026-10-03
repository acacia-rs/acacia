use aes::Aes256;
use bytes::BytesMut;
use ctr::cipher::{KeyIvInit, StreamCipher};
use sha2::{Digest, Sha256};

use crate::Error;

type Cipher = ctr::Ctr32BE<Aes256>;

pub const CHECKSUM_LEN: usize = 8;

/// Bedrock batch encryption: AES-256 in CTR mode (a GCM-style IV without authentication) plus a
/// truncated SHA-256 checksum keyed by a per-direction packet counter.
pub struct Encryption {
    key: [u8; 32],
    send: Cipher,
    recv: Cipher,
    send_counter: u64,
    recv_counter: u64,
}

impl Encryption {
    pub fn new(key: [u8; 32]) -> Self {
        let mut iv = [0u8; 16];
        iv[..12].copy_from_slice(&key[..12]);
        iv[15] = 2;
        Self {
            send: Cipher::new(&key.into(), &iv.into()),
            recv: Cipher::new(&key.into(), &iv.into()),
            key,
            send_counter: 0,
            recv_counter: 0,
        }
    }

    /// Appends the checksum of `buf[start..]` and encrypts that region in place.
    pub fn seal(&mut self, buf: &mut BytesMut, start: usize) {
        let sum = checksum(self.send_counter, &buf[start..], &self.key);
        self.send_counter += 1;
        buf.extend_from_slice(&sum);
        self.send.apply_keystream(&mut buf[start..]);
    }

    /// Decrypts `data` in place, verifies its checksum and returns the plaintext length.
    pub fn open(&mut self, data: &mut [u8]) -> Result<usize, Error> {
        self.recv.apply_keystream(data);
        let len = data.len().checked_sub(CHECKSUM_LEN).ok_or(Error::BadChecksum)?;
        let sum = checksum(self.recv_counter, &data[..len], &self.key);
        self.recv_counter += 1;
        if sum != data[len..] {
            return Err(Error::BadChecksum);
        }
        Ok(len)
    }
}

fn checksum(counter: u64, payload: &[u8], key: &[u8; 32]) -> [u8; CHECKSUM_LEN] {
    let mut h = Sha256::new();
    h.update(counter.to_le_bytes());
    h.update(payload);
    h.update(key);
    h.finalize()[..CHECKSUM_LEN].try_into().expect("sha256 is 32 bytes")
}

/// Session key = SHA-256(salt || ECDH(client secret, server public)).
pub fn derive_key(client: &p384::ecdsa::SigningKey, server: &p384::PublicKey, salt: &[u8]) -> [u8; 32] {
    let shared = p384::ecdh::diffie_hellman(client.as_nonzero_scalar(), server.as_affine());
    let mut h = Sha256::new();
    h.update(salt);
    h.update(shared.raw_secret_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_then_open_roundtrips_across_packets() {
        let key = [7u8; 32];
        let (mut client, mut server) = (Encryption::new(key), Encryption::new(key));
        for msg in [&b"first"[..], b"second packet", b""] {
            let mut buf = BytesMut::from(&[0xfe][..]);
            buf.extend_from_slice(msg);
            client.seal(&mut buf, 1);
            let n = server.open(&mut buf[1..]).unwrap();
            assert_eq!(&buf[1..1 + n], msg);
        }
    }

    #[test]
    fn tampered_payload_fails_checksum() {
        let key = [1u8; 32];
        let (mut client, mut server) = (Encryption::new(key), Encryption::new(key));
        let mut buf = BytesMut::from(&b"xhello"[..]);
        client.seal(&mut buf, 1);
        buf[2] ^= 1;
        assert!(matches!(server.open(&mut buf[1..]), Err(Error::BadChecksum)));
    }

    #[test]
    fn both_sides_derive_the_same_key() {
        let a = p384::ecdsa::SigningKey::from_bytes(&[3u8; 48].into()).unwrap();
        let b = p384::ecdsa::SigningKey::from_bytes(&[5u8; 48].into()).unwrap();
        let a_pub = p384::PublicKey::from(a.verifying_key());
        let b_pub = p384::PublicKey::from(b.verifying_key());
        assert_eq!(derive_key(&a, &b_pub, b"salt"), derive_key(&b, &a_pub, b"salt"));
    }
}
