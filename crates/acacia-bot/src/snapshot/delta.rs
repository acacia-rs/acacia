//! Entity list changes for readers that keep their own copy ([`crate::snapshot::SnapshotReader::entity_delta`]):
//!
//! - Each published entity has a **slot**, a small integer it keeps while it stays listed; a slot is reused once
//!   its entity has left. Slots are below [`EntityList::slot_capacity`], so a reader can keep numbers in an array.
//! - Each publish logs what changed against the list before it: entities added, entities removed (as last
//!   published), and entities whose numbers changed. A reader's **cursor** is the [`EntityList::version`] it holds;
//!   [`EntityDelta`] turns it into the current list. Apply `removed`, then `added`, then take the numbers of
//!   `added` and `moved` from the list. Changes on the way merge: an entity that came and went is left out.
//! - The log keeps at most [`MAX_VERSIONS`] publishes and [`MAX_ITEMS`] changes. A reader without a cursor, or one
//!   older than the log, gets a **full** delta: every entity in `added`, nothing removed, so it drops what it
//!   holds that is not re-added (by slot and identity).

use std::collections::VecDeque;
use std::sync::Arc;

use super::entity::{EntityInfo, EntityList};

/// Publishes the log keeps (a reader that fell further behind resyncs).
pub const MAX_VERSIONS: usize = 64;
/// Changes the log keeps across its publishes.
pub const MAX_ITEMS: usize = 16_384;

/// One publish's changes, by slot.
#[derive(Debug, Default)]
pub(crate) struct Changes {
    pub added: Vec<u32>,
    pub moved: Vec<u32>,
    pub removed: Vec<(u32, EntityInfo)>,
}

impl Changes {
    fn len(&self) -> usize {
        self.added.len() + self.moved.len() + self.removed.len()
    }
}

/// From a reader's cursor to [`EntityDelta::list`].
#[derive(Debug, Clone)]
pub struct EntityDelta {
    /// The list the reader now holds; its `version` is the next cursor.
    pub list: Arc<EntityList>,
    /// The reader starts over: `added` is every entity.
    pub full: bool,
    /// Slots and the entities that left them, as last published, by slot.
    pub removed: Vec<(u32, EntityInfo)>,
    /// Indices into `list.entities` of the entities that came, by slot.
    pub added: Vec<usize>,
    /// Indices into `list.entities` of entities the reader holds whose numbers changed, by slot.
    pub moved: Vec<usize>,
}

impl EntityDelta {
    fn full(list: &Arc<EntityList>) -> Self {
        let added = (0..list.slot_capacity()).filter_map(|s| list.index_of_slot(s)).collect();
        EntityDelta { list: list.clone(), full: true, removed: Vec::new(), added, moved: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        !self.full && self.removed.is_empty() && self.added.is_empty() && self.moved.is_empty()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChangeLog {
    /// The oldest cursor the log can serve.
    floor: u64,
    entries: VecDeque<(u64, Changes)>,
    items: usize,
}

impl ChangeLog {
    pub(crate) fn push(&mut self, version: u64, changes: Changes) {
        if changes.len() == 0 {
            return;
        }
        self.items += changes.len();
        self.entries.push_back((version, changes));
        while self.entries.len() > 1 && (self.entries.len() > MAX_VERSIONS || self.items > MAX_ITEMS) {
            let (version, dropped) = self.entries.pop_front().expect("more than one entry");
            self.items -= dropped.len();
            self.floor = version;
        }
    }

    /// `list` is the current list, the one the last [`ChangeLog::push`] was for.
    pub(crate) fn delta(&self, list: &Arc<EntityList>, since: Option<u64>) -> EntityDelta {
        match since {
            Some(since) if (self.floor..=list.version).contains(&since) => self.since(list, since),
            _ => EntityDelta::full(list),
        }
    }

    fn since(&self, list: &Arc<EntityList>, since: u64) -> EntityDelta {
        const ADDED: u8 = 1;
        const MOVED: u8 = 2;
        const GONE: u8 = 4;
        let mut flags = vec![0u8; list.slot_capacity() as usize];
        let mut removed = Vec::new();
        let start = self.entries.partition_point(|(v, _)| *v <= since);
        for (_, changes) in self.entries.range(start..) {
            for (slot, info) in &changes.removed {
                let f = &mut flags[*slot as usize];
                // Added after the cursor: the reader never saw it.
                if *f & ADDED == 0 && *f & GONE == 0 {
                    *f |= GONE;
                    removed.push((*slot, info.clone()));
                }
                *f &= !(ADDED | MOVED);
            }
            changes.added.iter().for_each(|&s| flags[s as usize] |= ADDED);
            changes.moved.iter().for_each(|&s| flags[s as usize] |= MOVED);
        }
        removed.sort_unstable_by_key(|(slot, _)| *slot);
        let (mut added, mut moved) = (Vec::new(), Vec::new());
        for (slot, f) in flags.iter().enumerate() {
            let Some(i) = list.index_of_slot(slot as u32) else { continue };
            if f & ADDED != 0 {
                added.push(i);
            } else if f & MOVED != 0 {
                moved.push(i);
            }
        }
        EntityDelta { list: list.clone(), full: false, removed, added, moved }
    }
}

#[cfg(test)]
#[path = "delta_tests.rs"]
mod tests;
