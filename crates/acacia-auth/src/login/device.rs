//! The Android phone a bot claims to be. Derived from the account, so each bot keeps one device
//! across joins (the vanilla client reuses its DeviceId and ClientRandomId) while bots differ.

use sha2::{Digest, Sha256};

/// `(DeviceModel, MemoryTier)`: Build.MANUFACTURER upper-cased plus Build.MODEL, as the Android
/// client sends it, with the tier the phone's RAM implies (3 = up to 8 GB, 4 = more).
const PHONES: [(&str, i32); 8] = [
    ("SAMSUNG SM-S918B", 4),
    ("SAMSUNG SM-S928B", 4),
    ("SAMSUNG SM-S911B", 3),
    ("SAMSUNG SM-S921B", 3),
    ("SAMSUNG SM-A546B", 3),
    ("SAMSUNG SM-A556B", 3),
    ("SAMSUNG SM-G991B", 3),
    ("SAMSUNG SM-A346B", 3),
];

pub(crate) struct Device {
    /// 32 lower-case hex digits with UUIDv4 version/variant bits (Android's format).
    pub device_id: String,
    pub client_random_id: i64,
    pub model: &'static str,
    pub memory_tier: i32,
}

impl Device {
    /// The device for an account (its display name, or the offline name).
    pub fn for_account(account: &str) -> Self {
        // Salt predates the Acacia rename; changing it gives every account a new device.
        let digest = Sha256::digest(format!("bedrock-client android device:{account}"));
        let mut id: [u8; 16] = digest[..16].try_into().expect("16 bytes");
        id[6] = (id[6] & 0x0f) | 0x40;
        id[8] = (id[8] & 0x3f) | 0x80;
        let random = i64::from_le_bytes(digest[16..24].try_into().expect("8 bytes"));
        let (model, memory_tier) = PHONES[usize::from(digest[24]) % PHONES.len()];
        Self { device_id: hex::encode(id), client_random_id: random & i64::MAX, model, memory_tier }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_per_account_and_distinct_between_accounts() {
        let (a, again, b) = (Device::for_account("Steve"), Device::for_account("Steve"), Device::for_account("Alex"));
        assert_eq!((a.device_id.as_str(), a.client_random_id), (again.device_id.as_str(), again.client_random_id));
        assert_ne!(a.device_id, b.device_id);
        assert_eq!(a.device_id.len(), 32);
        assert!(a.device_id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(&a.device_id[12..13], "4", "UUIDv4 version nibble");
        assert!(a.client_random_id >= 0);
        assert!(a.model.split(' ').next().is_some_and(|m| m.chars().all(|c| c.is_ascii_uppercase())));
    }
}
