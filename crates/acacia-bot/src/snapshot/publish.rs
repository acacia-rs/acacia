use std::sync::Arc;

use acacia_world::World;

use super::entity::{build_list, Interner};
use super::{build, SnapshotReader};
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
        let entities = Some(s.entities.version());
        if entities != p.entities_source {
            p.entities_source = entities;
            p.entities_version += 1;
            *p.reader.0.entities.lock() = Arc::new(build_list(&s.entities, p.entities_version, &mut p.interner));
        }
        let world = self.world.as_ref().and_then(|w| w.view()).map(|v| v.world());
        if world.map(Arc::as_ptr) != p.world.as_ref().map(Arc::as_ptr) {
            p.world = world.cloned();
            *p.reader.0.world.lock() = p.world.clone();
        }
        self.snapshots = Some(p);
    }
}
