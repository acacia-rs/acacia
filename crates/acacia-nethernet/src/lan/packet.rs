//! LAN discovery datagrams: `HMAC-SHA256(plaintext) ‖ AES-256-ECB-PKCS7(plaintext)`, both keyed with
//! `sha256(0xdeadbeef as u64 LE)`. Plaintext: `u16 len ‖ u16 id ‖ u64 sender ‖ 8×0 ‖ body` (LE).
//! Spec: docs/research/nethernet-signaling.md §1a (layout follows go-nethernet `discovery`, MIT).

use std::sync::OnceLock;

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::Error;

const ID_REQUEST: u16 = 0;
const ID_RESPONSE: u16 = 1;
const ID_MESSAGE: u16 = 2;
const MAC_LEN: usize = 32;
const BLOCK: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanPacket {
    /// Broadcast by clients looking for worlds.
    Request,
    /// A host's answer: its ServerData bytes (hex on the wire).
    Response(Vec<u8>),
    /// Signal text for `recipient` (see [`crate::Signal`]).
    Message { recipient: u64, data: String },
}

fn key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| Sha256::digest(0xdead_beef_u64.to_le_bytes()).into())
}

fn mac(plaintext: &[u8]) -> [u8; MAC_LEN] {
    let mut h = <Hmac<Sha256> as Mac>::new_from_slice(key()).expect("any key length");
    h.update(plaintext);
    h.finalize().into_bytes().into()
}

impl LanPacket {
    pub fn encode(&self, sender: u64) -> Vec<u8> {
        let mut p = vec![0, 0];
        let id = match self {
            Self::Request => ID_REQUEST,
            Self::Response(_) => ID_RESPONSE,
            Self::Message { .. } => ID_MESSAGE,
        };
        p.extend_from_slice(&id.to_le_bytes());
        p.extend_from_slice(&sender.to_le_bytes());
        p.extend_from_slice(&[0; 8]);
        match self {
            Self::Request => {}
            Self::Response(data) => put_bytes(&mut p, hex(data).as_bytes()),
            Self::Message { recipient, data } => {
                p.extend_from_slice(&recipient.to_le_bytes());
                put_bytes(&mut p, data.as_bytes());
            }
        }
        // TODO: go-nethernet counts the length field itself, node-nethernet doesn't; confirm by capture.
        let len = p.len() as u16;
        p[..2].copy_from_slice(&len.to_le_bytes());
        let mut out = mac(&p).to_vec();
        out.extend(encrypt(&p));
        out
    }

    /// Decodes a datagram and returns it with its sender id.
    pub fn decode(datagram: &[u8]) -> Result<(Self, u64), Error> {
        let bad = |why: &str| Error::Lan(why.to_owned());
        if datagram.len() < MAC_LEN {
            return Err(bad("short datagram"));
        }
        let p = decrypt(&datagram[MAC_LEN..]).ok_or_else(|| bad("bad ciphertext"))?;
        if mac(&p) != datagram[..MAC_LEN] {
            return Err(bad("checksum mismatch"));
        }
        // The length field is ignored on read, as go-nethernet does.
        let mut r = Reader(p.get(2..).ok_or_else(|| bad("short header"))?);
        let id = u16::from_le_bytes(r.take(2)?.try_into().expect("2 bytes"));
        let sender = r.u64()?;
        r.take(8)?;
        let packet = match id {
            ID_REQUEST => Self::Request,
            ID_RESPONSE => Self::Response(unhex(r.bytes()?).ok_or_else(|| bad("response is not hex"))?),
            ID_MESSAGE => {
                let recipient = r.u64()?;
                // Vanilla hosts under-declare CONNECTRESPONSE lengths: the rest of the datagram is data.
                let mut data = r.bytes()?.to_vec();
                data.extend_from_slice(r.0);
                r.0 = &[];
                Self::Message { recipient, data: String::from_utf8(data).map_err(|_| bad("message is not UTF-8"))? }
            }
            other => return Err(Error::Lan(format!("unknown packet id {other}"))),
        };
        if !r.0.is_empty() {
            return Err(bad("trailing bytes"));
        }
        Ok((packet, sender))
    }
}

fn put_bytes(p: &mut Vec<u8>, b: &[u8]) {
    p.extend_from_slice(&(b.len() as u32).to_le_bytes());
    p.extend_from_slice(b);
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.0.len() < n {
            return Err(Error::Lan("truncated packet".into()));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes")))
    }

    /// A u32-length-prefixed field, clamped to what is left (see the message quirk above).
    fn bytes(&mut self) -> Result<&'a [u8], Error> {
        let n = u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")) as usize;
        self.take(n.min(self.0.len()))
    }
}

fn encrypt(plain: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(key().into());
    let pad = BLOCK - plain.len() % BLOCK;
    let mut buf = plain.to_vec();
    buf.resize(plain.len() + pad, pad as u8);
    for block in buf.as_chunks_mut::<BLOCK>().0 {
        cipher.encrypt_block(block.into());
    }
    buf
}

fn decrypt(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() || data.len() % BLOCK != 0 {
        return None;
    }
    let cipher = Aes256::new(key().into());
    let mut buf = data.to_vec();
    for block in buf.as_chunks_mut::<BLOCK>().0 {
        cipher.decrypt_block(block.into());
    }
    let pad = *buf.last()? as usize;
    if pad == 0 || pad > BLOCK || !buf[buf.len() - pad..].iter().all(|b| *b as usize == pad) {
        return None;
    }
    buf.truncate(buf.len() - pad);
    Some(buf)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &[u8]) -> Option<Vec<u8>> {
    let s = std::str::from_utf8(s).ok()?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_packet() {
        let sender = 0x1020304050607080;
        for p in [
            LanPacket::Request,
            LanPacket::Response(vec![7, 1, 2, 0xff]),
            LanPacket::Message { recipient: 42, data: "CONNECTREQUEST 1 v=0\r\n".into() },
        ] {
            assert_eq!(LanPacket::decode(&p.encode(sender)).unwrap(), (p, sender));
        }
    }

    #[test]
    fn request_layout_and_key() {
        // A Request is 20 plaintext bytes: one padded block pair after the 32-byte MAC.
        let d = LanPacket::Request.encode(1);
        assert_eq!(d.len(), 32 + 32);
        let plain = decrypt(&d[32..]).unwrap();
        assert_eq!(plain, [20, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(hex(&key()[..4]), hex(&Sha256::digest([0xef, 0xbe, 0xad, 0xde, 0, 0, 0, 0])[..4]));
    }

    #[test]
    fn rejects_tampering_and_reads_under_declared_messages() {
        let mut d = LanPacket::Request.encode(1);
        d[0] ^= 1;
        assert!(LanPacket::decode(&d).is_err());

        let mut p = vec![0, 0, ID_MESSAGE as u8, 0];
        p.extend_from_slice(&9u64.to_le_bytes());
        p.extend_from_slice(&[0; 8]);
        p.extend_from_slice(&5u64.to_le_bytes());
        p.extend_from_slice(&3u32.to_le_bytes());
        p.extend_from_slice(b"CONNECTRESPONSE 1 v=0");
        let mut datagram = mac(&p).to_vec();
        datagram.extend(encrypt(&p));
        let (packet, sender) = LanPacket::decode(&datagram).unwrap();
        assert_eq!((packet, sender), (LanPacket::Message { recipient: 5, data: "CONNECTRESPONSE 1 v=0".into() }, 9));
    }
}
