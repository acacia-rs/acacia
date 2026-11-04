use std::sync::Arc;

use acacia_world::World;

use super::slots::ListBuilder;
use super::{build, SnapshotReader};
use crate::state::Entities;
use crate::Bot;

/// The bot's side of [`SnapshotReader`]: what it last published.
#[derive(Default)]
pub(crate) struct Publisher {
    reader: SnapshotReader,
    pub(super) tick: u64,
    /// Inventory, effects and player-list versions the published cold part was built from.
    sources: Option<(u64, u64, u64)>,
    cold_version: u64,
    /// The entity tracker's version the published list was built from.
    entities_source: Option<u64>,
    entities_version: u64,
    lists: ListBuilder,
    /// Scratch for [`Entities::take_touched`].
    touched: Vec<u64>,
    world: Option<Arc<World>>,
}

impl Bot {
    /// Reads the snapshots this bot publishes; `None` unless [`crate::BotConfig::snapshots`] is set.
    pub fn snapshot_reader(&self) -> Option<SnapshotReader> {
        self.snapshots.as_ref().map(|p| p.reader.clone())
    }

    /// After the ticks that came due (see [`crate::snapshot`]).
    pub(crate) fn publish_snapshot(&mut self, ticks: u32) {
        let Some(mut p) = self.snapshots.take() else { return };
        p.tick += u64::from(ticks);
        *p.reader.0.hot.lock() = self.hot_snapshot(p.tick);
        p.publish_entities(&mut self.state.entities);
        let s = &self.state;
        let sources = Some((s.inventory.version(), s.player.effects.version(), s.player_list.version()));
        if sources != p.sources {
            p.sources = sources;
            p.cold_version += 1;
            *p.reader.0.cold.lock() = Arc::new(build::build_cold(s, p.cold_version));
        }
        let mut items = p.reader.0.items.lock();
        if !s.items.same(&items) {
            *items = s.items.clone();
        }
        drop(items);
        let world = self.world.as_ref().and_then(|w| w.view()).map(|v| v.world());
        if world.map(Arc::as_ptr) != p.world.as_ref().map(Arc::as_ptr) {
            p.world = world.cloned();
            *p.reader.0.world.lock() = p.world.clone();
        }
        self.snapshots = Some(p);
    }

    /// Before entity events go out, so a reader sees the entity an event names (see [`crate::snapshot`]).
    pub(crate) fn publish_entity_changes(&mut self) {
        if let Some(p) = &mut self.snapshots
            && !self.state.entities.changes.is_empty()
        {
            p.publish_entities(&mut self.state.entities);
        }
    }
}

impl Publisher {
    /// Usually only some entities moved and none came or went: those are patched into the list in place.
    fn publish_entities(&mut self, entities: &mut Entities) {
        let source = Some(entities.listed_version());
        if source == self.entities_source {
            return;
        }
        self.entities_source = source;
        let listed_all = entities.take_touched(&mut self.touched);
        if listed_all && self.lists.can_patch(entities) {
            let mut guard = self.reader.0.entities.lock();
            let published = &mut *guard;
            // In place unless a reader still holds the list.
            let list = Arc::make_mut(&mut published.list);
            let changes = self.lists.patch(entities, &self.touched, list);
            if !changes.moved.is_empty() {
                self.entities_version += 1;
                list.version = self.entities_version;
                published.log.push(list.version, changes);
            }
            return;
        }
        self.entities_version += 1;
        let previous = self.reader.entities();
        let (list, changes) = self.lists.build(entities, self.entities_version, &previous);
        let mut published = self.reader.0.entities.lock();
        published.log.push(list.version, changes);
        published.list = Arc::new(list);
    }
}

#[cfg(test)]
#[path = "publish_tests.rs"]
mod tests;
