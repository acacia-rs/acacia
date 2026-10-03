/// Which packet IDs a [`crate::Client`] delivers. Packets filtered out are dropped inside the
/// driver without being decoded or queued, which is most of the cost saving for idle bots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketFilter([u64; 16]);

impl PacketFilter {
    pub fn all() -> Self {
        Self([u64::MAX; 16])
    }

    pub fn none() -> Self {
        Self([0; 16])
    }

    pub fn with(mut self, id: u32) -> Self {
        let id = id & 0x3ff;
        self.0[(id / 64) as usize] |= 1 << (id % 64);
        self
    }

    pub fn union(mut self, other: &PacketFilter) -> Self {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a |= b;
        }
        self
    }

    pub fn allows(&self, id: u32) -> bool {
        let id = id & 0x3ff;
        self.0[(id / 64) as usize] & (1 << (id % 64)) != 0
    }
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self::all()
    }
}

impl FromIterator<u32> for PacketFilter {
    fn from_iter<I: IntoIterator<Item = u32>>(ids: I) -> Self {
        ids.into_iter().fold(Self::none(), Self::with)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribed_ids_only() {
        let f: PacketFilter = [10, 63, 64, 1023].into_iter().collect();
        assert!(f.allows(10) && f.allows(63) && f.allows(64) && f.allows(1023));
        assert!(!f.allows(11) && !f.allows(0));
        assert!(PacketFilter::all().allows(500));
    }
}
