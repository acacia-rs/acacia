//! Which tracked entities changed a field the snapshot list carries ([`crate::snapshot::EntityInfo`]: position,
//! motion, rotation, health), so the published list re-reads only those (`snapshot/publish.rs`).

use super::{Entities, Entity};

/// Distinct entities stamped between two takes before the list gives up and the taker rebuilds everything.
const MAX_TOUCHED: usize = 4096;

#[derive(Debug, Default)]
pub(super) struct Stamps {
    /// Moves on every listed change and every spawn or removal.
    version: u64,
    /// Moves on every spawn or removal.
    members: u64,
    /// `version` at the last take.
    taken: u64,
    /// Runtime ids stamped since the last take, each once.
    touched: Vec<u64>,
    overflowed: bool,
}

impl Stamps {
    /// `e`'s listed fields changed (or may have).
    pub(super) fn touch(&mut self, e: &mut Entity) {
        if e.listed_at <= self.taken && !self.overflowed {
            self.overflowed = self.touched.len() >= MAX_TOUCHED;
            match self.overflowed {
                true => self.touched.clear(),
                false => self.touched.push(e.runtime_id),
            }
        }
        self.version += 1;
        e.listed_at = self.version;
    }

    pub(super) fn members_changed(&mut self) {
        self.version += 1;
        self.members += 1;
    }
}

impl Entities {
    /// Moves when a listed field of a tracked entity changed, or an entity came or went.
    pub fn listed_version(&self) -> u64 {
        self.stamps.version
    }

    /// Moves when an entity came or went.
    pub fn members_version(&self) -> u64 {
        self.stamps.members
    }

    /// Swaps the entities stamped since the last take into `into`; false when too many piled up to list.
    pub(crate) fn take_touched(&mut self, into: &mut Vec<u64>) -> bool {
        into.clear();
        std::mem::swap(into, &mut self.stamps.touched);
        self.stamps.taken = self.stamps.version;
        !std::mem::take(&mut self.stamps.overflowed)
    }
}
