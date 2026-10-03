//! Hand-written types that the generator references by name (see docs/proto.md).

use std::fmt;
use std::sync::atomic::{AtomicI32, Ordering};

/// 16 raw wire bytes. Bedrock writes a UUID as two little-endian u64 halves, so each half is the
/// canonical bytes reversed; `Display`/`FromStr` convert to and from the canonical string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Uuid(pub [u8; 16]);

/// Wire order <-> canonical order (an involution: each 8-byte half reversed).
fn swap_halves(mut b: [u8; 16]) -> [u8; 16] {
    b[..8].reverse();
    b[8..].reverse();
    b
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, b) in swap_halves(self.0).iter().enumerate() {
            if matches!(i, 4 | 6 | 8 | 10) {
                f.write_str("-")?;
            }
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for Uuid {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let hex: Vec<u8> = s.bytes().filter(|&c| c != b'-').collect();
        if hex.len() != 32 {
            return Err(());
        }
        let mut canonical = [0u8; 16];
        for (i, pair) in hex.chunks(2).enumerate() {
            let s = std::str::from_utf8(pair).map_err(|_| ())?;
            canonical[i] = u8::from_str_radix(s, 16).map_err(|_| ())?;
        }
        Ok(Uuid(swap_halves(canonical)))
    }
}

#[cfg(test)]
mod tests {
    use super::Uuid;

    #[test]
    fn uuid_strings_match_the_wire_layout() {
        // The vanilla client's "wave" emote, as captured on the wire.
        let wire = [0xcd, 0x47, 0x2e, 0xdf, 0x10, 0xe7, 0x8a, 0x4c, 0x67, 0x3d, 0x1a, 0xf2, 0x7b, 0xcc, 0x4d, 0x81];
        let uuid: Uuid = "4c8ae710-df2e-47cd-814d-cc7bf21a3d67".parse().unwrap();
        assert_eq!(uuid.0, wire);
        assert_eq!(uuid.to_string(), "4c8ae710-df2e-47cd-814d-cc7bf21a3d67");
    }
}

/// Vanilla shield runtime id; items with this network id carry an extra `blocking_tick` field.
pub const DEFAULT_SHIELD_ITEM_ID: i32 = 387;

// Process-wide, like bedrock-protocol's `/ShieldItemID`: the only item-encoding input that comes from the server's item registry.
static SHIELD_ITEM_ID: AtomicI32 = AtomicI32::new(DEFAULT_SHIELD_ITEM_ID);

/// Network id of `minecraft:shield`, as announced in the server's item registry.
pub fn shield_item_id() -> i32 {
    SHIELD_ITEM_ID.load(Ordering::Relaxed)
}

pub fn set_shield_item_id(id: i32) {
    SHIELD_ITEM_ID.store(id, Ordering::Relaxed);
}
