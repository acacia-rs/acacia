//! Stateless handshake cookies. Reply 1 hands the client `SipHash(secret, period ‖ address)`
//! and request 2 must echo it, so only someone who receives datagrams at that address can open
//! (or disturb) a connection from it. The period makes a captured cookie expire.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// A cookie is accepted in the period it was issued in and the next one.
const PERIOD: Duration = Duration::from_secs(10);

/// 128 bits from the OS, by way of the keys std draws for its hash maps (the crate has no RNG).
pub(crate) fn random_secret() -> u128 {
    let word = || u128::from(RandomState::new().build_hasher().finish());
    word() << 64 | word()
}

pub(crate) struct Cookies {
    key: [u64; 2],
}

impl Cookies {
    pub fn new(secret: u128) -> Self {
        Self { key: [secret as u64, (secret >> 64) as u64] }
    }

    fn at(&self, addr: SocketAddr, period: u64) -> u32 {
        let ip = match addr.ip() {
            IpAddr::V4(ip) => ip.to_ipv6_mapped(),
            IpAddr::V6(ip) => ip,
        };
        let mut msg = [0u8; 26];
        msg[..8].copy_from_slice(&period.to_le_bytes());
        msg[8..24].copy_from_slice(&ip.octets());
        msg[24..].copy_from_slice(&addr.port().to_le_bytes());
        siphash24(self.key, &msg) as u32
    }

    /// `age` is the time since the server's epoch.
    pub fn issue(&self, addr: SocketAddr, age: Duration) -> u32 {
        self.at(addr, age.as_secs() / PERIOD.as_secs())
    }

    pub fn verify(&self, addr: SocketAddr, age: Duration, cookie: u32) -> bool {
        let period = age.as_secs() / PERIOD.as_secs();
        cookie == self.at(addr, period) || period.checked_sub(1).is_some_and(|p| cookie == self.at(addr, p))
    }
}

fn sip_round(v: &mut [u64; 4]) {
    v[0] = v[0].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(13) ^ v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16) ^ v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21) ^ v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17) ^ v[2];
    v[2] = v[2].rotate_left(32);
}

/// SipHash-2-4 (Aumasson and Bernstein), a keyed PRF; std exposes none with a caller-chosen key.
fn siphash24(key: [u64; 2], data: &[u8]) -> u64 {
    let mut v = [key[0] ^ 0x736f_6d65_7073_6575, key[1] ^ 0x646f_7261_6e64_6f6d, key[0] ^ 0x6c79_6765_6e65_7261, key[1] ^ 0x7465_6462_7974_6573];
    let mut absorb = |m: u64| {
        v[3] ^= m;
        sip_round(&mut v);
        sip_round(&mut v);
        v[0] ^= m;
    };
    let mut words = data.chunks_exact(8);
    for word in &mut words {
        absorb(u64::from_le_bytes(word.try_into().expect("8 bytes")));
    }
    let mut last = [0u8; 8];
    last[..words.remainder().len()].copy_from_slice(words.remainder());
    last[7] = data.len() as u8;
    absorb(u64::from_le_bytes(last));
    v[2] ^= 0xff;
    for _ in 0..4 {
        sip_round(&mut v);
    }
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn siphash_matches_the_reference_vector() {
        let bytes: Vec<u8> = (0..16).collect();
        let key = [u64::from_le_bytes(bytes[..8].try_into().unwrap()), u64::from_le_bytes(bytes[8..].try_into().unwrap())];
        assert_eq!(siphash24(key, &bytes[..15]), 0xa129_ca61_49be_45e5);
    }

    #[test]
    fn cookies_bind_the_address_and_expire() {
        let cookies = Cookies::new(1);
        let (a, b): (SocketAddr, SocketAddr) = ("10.0.0.2:5000".parse().unwrap(), "10.0.0.2:5001".parse().unwrap());
        let issued = Duration::from_secs(25);
        let cookie = cookies.issue(a, issued);
        assert!(cookies.verify(a, issued, cookie));
        assert!(cookies.verify(a, issued + PERIOD, cookie), "still good in the next period");
        assert!(!cookies.verify(a, issued + PERIOD * 2, cookie));
        assert!(!cookies.verify(b, issued, cookie), "another port is another address");
        assert!(!Cookies::new(2).verify(a, issued, cookie), "another server's secret");
        assert_ne!(random_secret(), random_secret());
    }
}
