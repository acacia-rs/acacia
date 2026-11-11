//! Protocol encryption: AES-128/CFB8 keyed and seeded with the shared secret, RSA key exchange, the session hash.

use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit};
use rsa::pkcs8::DecodePublicKey;
use rsa::rand_core::OsRng;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use sha1::{Digest, Sha1};

use crate::frame::Error;

/// One direction of the stream.
pub(crate) struct Cfb8 {
    cipher: Aes128,
    iv: [u8; 16],
}

impl Cfb8 {
    pub(crate) fn new(secret: &[u8; 16]) -> Self {
        Self { cipher: Aes128::new(secret.into()), iv: *secret }
    }

    fn keystream_byte(&self) -> u8 {
        let mut block = self.iv.into();
        self.cipher.encrypt_block(&mut block);
        block[0]
    }

    fn shift_in(&mut self, ciphertext: u8) {
        self.iv.copy_within(1.., 0);
        self.iv[15] = ciphertext;
    }

    pub(crate) fn encrypt(&mut self, data: &mut [u8]) {
        for byte in data {
            *byte ^= self.keystream_byte();
            self.shift_in(*byte);
        }
    }

    pub(crate) fn decrypt(&mut self, data: &mut [u8]) {
        for byte in data {
            let ciphertext = *byte;
            *byte ^= self.keystream_byte();
            self.shift_in(ciphertext);
        }
    }
}

/// Encrypts with the server's public key (X.509 DER), as the login `Key` packet requires.
pub(crate) fn rsa_encrypt(public_key: &[u8], data: &[u8]) -> Result<Vec<u8>, Error> {
    let key = RsaPublicKey::from_public_key_der(public_key).map_err(|e| Error::Crypto(e.to_string()))?;
    key.encrypt(&mut OsRng, Pkcs1v15Encrypt, data).map_err(|e| Error::Crypto(e.to_string()))
}

pub(crate) fn random_secret() -> [u8; 16] {
    use rsa::rand_core::RngCore;
    let mut secret = [0; 16];
    OsRng.fill_bytes(&mut secret);
    secret
}

/// `Crypt.digestData` printed as Java's `BigInteger.toString(16)`: a signed value without leading zeros.
pub(crate) fn server_hash(server_id: &str, secret: &[u8], public_key: &[u8]) -> String {
    signed_hex(Sha1::new().chain_update(server_id.as_bytes()).chain_update(secret).chain_update(public_key).finalize().into())
}

fn signed_hex(mut digest: [u8; 20]) -> String {
    let negative = digest[0] & 0x80 != 0;
    if negative {
        let mut carry = true;
        for byte in digest.iter_mut().rev() {
            let (value, overflow) = (!*byte).overflowing_add(u8::from(carry));
            (*byte, carry) = (value, overflow);
        }
    }
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let trimmed = hex.trim_start_matches('0');
    format!("{}{trimmed}", if negative { "-" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(name: &str) -> [u8; 20] {
        Sha1::digest(name.as_bytes()).into()
    }

    #[test]
    fn hashes_print_like_java_big_integers() {
        assert_eq!(signed_hex(digest("Notch")), "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
        assert_eq!(signed_hex(digest("jeb_")), "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
        assert_eq!(signed_hex(digest("simon")), "88e16a1019277b15d58faf0541e11910eb756f6");
    }

    #[test]
    fn cfb8_matches_nist_sp800_38a() {
        let key = [0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c];
        let iv: [u8; 16] = std::array::from_fn(|i| i as u8);
        let plain = [0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17, 0x2a, 0xae, 0x2d];
        let expected = [0x3b, 0x79, 0x42, 0x4c, 0x9c, 0x0d, 0xd4, 0x36, 0xba, 0xce, 0x9e, 0x0e, 0xd4, 0x58, 0x6a, 0x4f, 0x32, 0xb9];
        let make = || Cfb8 { cipher: Aes128::new((&key).into()), iv };
        let mut data = plain;
        make().encrypt(&mut data);
        assert_eq!(data, expected);
        make().decrypt(&mut data);
        assert_eq!(data, plain);
    }
}
