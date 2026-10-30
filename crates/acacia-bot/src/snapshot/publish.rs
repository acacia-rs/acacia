use std::sync::Arc;

use acacia_world::World;

use super::entity::{build_list, Interner};
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
    interner: Interner,
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
        let s = &self.state;
        let sources = Some((s.inventory.version(), s.player.effects.version(), s.player_list.version()));
        if sources != p.sources {
            p.sources = sources;
            p.cold_version += 1;
            *p.reader.0.cold.lock() = Arc::new(build::build_cold(s, p.cold_version));
        }
        p.publish_entities(&s.entities);
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
            p.publish_entities(&self.state.entities);
        }
    }
}

impl Publisher {
    fn publish_entities(&mut self, entities: &Entities) {
        let source = Some(entities.version());
        if source != self.entities_source {
            self.entities_source = source;
            self.entities_version += 1;
            *self.reader.0.entities.lock() = Arc::new(build_list(entities, self.entities_version, &mut self.interner));
        }
    }
}
