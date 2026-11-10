//! Plain-data copies of a bot's state for other threads ([`SnapshotReader`]), off unless
//! [`crate::BotConfig::snapshots`] is set. The one place the contract is written down:
//! - [`Hot`] is copied every client tick: a few dozen scalars, no allocation.
//! - [`Cold`] (inventory, effects, player list) is rebuilt only when the version counters of
//!   [`crate::state::Inventory`], [`crate::state::Effects`] or [`crate::state::PlayerList`] move, and shared as an
//!   `Arc`; [`Cold::version`] changes exactly when it is rebuilt, so readers can cache on it.
//! - [`EntityList`] (tracked entities, feet positions) is updated once per tick batch when a listed field of a
//!   tracked entity changed ([`crate::state::Entities::listed_version`]), and at once on a spawn or removal,
//!   before its event goes out (a listener finds the entity listed). Without a spawn or removal only the entities
//!   the tracker stamped are re-read, in place unless a reader holds the list; otherwise it is rebuilt.
//!   [`EntityList::version`] changes exactly when the list does. Empty unless `Trackers::entities` (or
//!   `hazards`) is on. Readers that keep a copy take changes instead ([`SnapshotReader::entity_delta`]; slots,
//!   cursors and resyncs: [`delta`]).
//! - Stacks carry network ids, not names: resolve them through [`SnapshotReader::items`], the item table the server
//!   sends once (shared, not copied). Player strings are shared too.
//! - Blocks need no copy: [`SnapshotReader::world`] is the bot's current dimension, readable from any thread;
//!   it changes on a dimension change.
//! - [`crate::Bot::snapshot`] builds one on demand, published or not; so does [`crate::inbox::Immediate::Snapshot`].
//!   Measurements: `research/js-measure/snapshot-results.md` and `examples/snapshot_cost.rs`.

mod build;
pub mod delta;
mod entity;
mod publish;
mod slots;

use std::sync::Arc;

use acacia_client::proto::manual::Uuid;
use acacia_world::World;
use parking_lot::Mutex;

use crate::state::ItemRegistry;

pub use delta::EntityDelta;
pub use entity::{EntityInfo, EntityList};
pub(crate) use publish::Publisher;

/// Changes nearly every tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Hot {
    /// Client ticks since the bot connected (0 when snapshots are not published).
    pub tick: u64,
    pub position: [f32; 3],
    /// Physics bots only (zero otherwise), like `on_ground` and `gliding`.
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub saturation: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    pub air: i16,
    pub alive: bool,
    pub game_mode: i64,
    pub dimension: i32,
    pub held_slot: u8,
    pub open_window: Option<i32>,
    pub flying: bool,
    pub sleeping: bool,
    pub riding: bool,
    pub gliding: bool,
}

/// A non-empty stack, summarised (no NBT tree, no lore).
#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub window: i32,
    pub slot: u8,
    pub network_id: i32,
    pub count: u16,
    pub metadata: u32,
    pub stack_id: Option<i32>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectSummary {
    pub id: i32,
    pub amplifier: i32,
    pub duration: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub uuid: Uuid,
    pub username: Arc<str>,
    pub xuid: Arc<str>,
    pub entity_unique_id: i64,
}

/// Changes on inventory, effect and player-list updates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cold {
    /// 0 in an on-demand snapshot.
    pub version: u64,
    pub inventory: Vec<Stack>,
    pub cursor: Option<Stack>,
    pub effects: Vec<EffectSummary>,
    pub players: Vec<Player>,
}

#[derive(Debug, Clone)]
pub struct BotSnapshot {
    pub hot: Hot,
    pub cold: Arc<Cold>,
    pub entities: Arc<EntityList>,
}

/// Reads a bot's published snapshot from any thread (see [`crate::snapshot`]). Cheap to clone.
#[derive(Clone, Default)]
pub struct SnapshotReader(Arc<Slot>);

#[derive(Default)]
struct Slot {
    hot: Mutex<Hot>,
    cold: Mutex<Arc<Cold>>,
    entities: Mutex<PublishedEntities>,
    world: Mutex<Option<Arc<World>>>,
    items: Mutex<ItemRegistry>,
}

/// The list and the log of how it got there, under one lock so a delta always ends at the list.
#[derive(Default)]
struct PublishedEntities {
    list: Arc<EntityList>,
    log: delta::ChangeLog,
}

impl SnapshotReader {
    pub fn hot(&self) -> Hot {
        *self.0.hot.lock()
    }

    pub fn cold(&self) -> Arc<Cold> {
        self.0.cold.lock().clone()
    }

    pub fn entities(&self) -> Arc<EntityList> {
        self.0.entities.lock().list.clone()
    }

    /// The changes from the list whose version is `since` to the current one (see [`delta`]).
    pub fn entity_delta(&self, since: Option<u64>) -> EntityDelta {
        let published = self.0.entities.lock();
        published.log.delta(&published.list, since)
    }

    pub fn get(&self) -> BotSnapshot {
        BotSnapshot { hot: self.hot(), cold: self.cold(), entities: self.entities() }
    }

    /// The bot's terrain (`None` before StartGame); blocks via `World::get(cx, cz)` and the chunk's read lock.
    pub fn world(&self) -> Option<Arc<World>> {
        self.0.world.lock().clone()
    }

    /// The server's item table (network id to name), empty before it arrives; it comes once, at login.
    pub fn items(&self) -> ItemRegistry {
        self.0.items.lock().clone()
    }
}
