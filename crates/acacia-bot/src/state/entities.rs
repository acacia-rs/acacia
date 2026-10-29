use std::collections::HashMap;

use acacia_client::proto::manual::Uuid;
use acacia_client::proto::types::Vec3f;
use rustc_hash::FxHashMap;

use super::{Effects, ItemStack, Metadata};

mod apply;
mod equipment;
mod moves;
mod properties;
mod solids;

pub use equipment::Equipment;
pub use properties::{Property, Values as PropertyValues};

pub const PLAYER_KIND: &str = "minecraft:player";
pub const ITEM_KIND: &str = "minecraft:item";
/// Players' wire position is this far above their feet, whatever their pose.
pub const PLAYER_EYE_HEIGHT: f32 = super::PlayerState::EYE_HEIGHT;

const BYTE_ROTATION: f32 = 360.0 / 256.0;

/// One tracked entity or other player. Angles are in degrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub runtime_id: u64,
    pub unique_id: i64,
    /// Identifier such as `minecraft:zombie`; [`PLAYER_KIND`] for players, [`ITEM_KIND`] for dropped items.
    pub kind: String,
    pub username: Option<String>,
    /// Position as sent by the server: the eye position for players (see [`Entity::feet`]), feet otherwise.
    pub position: Vec3f,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    /// Last velocity from spawn or SetEntityMotion; not derived from movement.
    pub velocity: Vec3f,
    /// Blocks per tick between the last two position updates; a SetEntityMotion replaces it until the next one.
    pub motion: Vec3f,
    /// [`Entities::now`] when the position last changed (spawn included).
    pub moved_at: u64,
    pub on_ground: bool,
    /// Players only; the key into [`super::PlayerList`] and [`super::Skins`].
    pub uuid: Option<Uuid>,
    pub metadata: Metadata,
    /// Synced actor properties; read them through [`Entities::property`].
    pub properties: PropertyValues,
    /// Current attribute values by name (`minecraft:health`, `minecraft:movement`, ...).
    pub attributes: HashMap<String, f32>,
    pub effects: Effects,
    /// `None` until the server sends any.
    pub equipment: Option<Box<Equipment>>,
    /// The stack of a dropped item ([`ITEM_KIND`]).
    pub item: Option<ItemStack>,
}

impl Entity {
    fn spawned(runtime_id: u64, unique_id: i64, kind: String, position: Vec3f, velocity: Vec3f) -> Self {
        Entity {
            runtime_id,
            unique_id,
            kind,
            username: None,
            position,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            motion: velocity.clone(),
            velocity,
            moved_at: 0,
            on_ground: false,
            uuid: None,
            metadata: Metadata::default(),
            properties: PropertyValues::default(),
            attributes: HashMap::new(),
            effects: Effects::default(),
            equipment: None,
            item: None,
        }
    }

    pub fn is_player(&self) -> bool {
        self.kind == PLAYER_KIND
    }

    pub fn feet(&self) -> Vec3f {
        let p = &self.position;
        let dy = if self.is_player() { PLAYER_EYE_HEIGHT } else { 0.0 };
        Vec3f { x: p.x, y: p.y - dy, z: p.z }
    }

    pub fn distance_sq(&self, to: &Vec3f) -> f32 {
        let (dx, dy, dz) = (self.position.x - to.x, self.position.y - to.y, self.position.z - to.z);
        dx * dx + dy * dy + dz * dz
    }

    /// `minecraft:health` if the server sent it (mobs only; players' health is not broadcast).
    pub fn health(&self) -> Option<f32> {
        self.attributes.get("minecraft:health").copied()
    }
}

/// Entities by runtime id, excluding the local player. Off by default (see [`crate::BotConfig`]).
/// Fx-hashed and boxed: every entity move looks one up (~300 k/s across a 50-bot swarm among 300 mobs),
/// and a table of whole entities missed the cache on its buckets.
#[derive(Debug, Default)]
pub struct Entities {
    by_runtime: FxHashMap<u64, Box<Entity>>,
    runtime_by_unique: FxHashMap<i64, u64>,
    property_defs: properties::Definitions,
    /// Client ticks since tracking began.
    now: u64,
    /// Lean tracking ([`super::Trackers::hazards`]): only what hazard avoidance reads is kept.
    pub(crate) lean: bool,
}

impl Entities {
    pub fn get(&self, runtime_id: u64) -> Option<&Entity> {
        self.by_runtime.get(&runtime_id).map(|e| &**e)
    }

    /// Client ticks counted so far: the clock of [`Entity::moved_at`].
    pub fn now(&self) -> u64 {
        self.now
    }

    pub(crate) fn advance(&mut self) {
        self.now += 1;
    }

    /// Moves a tracked entity the bot itself simulates (a vehicle it drives, which the server does not
    /// send moves for).
    pub(crate) fn set_pose(&mut self, runtime_id: u64, feet: [f32; 3], yaw: f32, pitch: f32) {
        if let Some(e) = self.by_runtime.get_mut(&runtime_id) {
            let [x, y, z] = feet;
            e.position = Vec3f { x, y, z };
            (e.yaw, e.pitch) = (yaw, pitch);
        }
    }

    pub fn by_unique(&self, unique_id: i64) -> Option<&Entity> {
        self.runtime_by_unique.get(&unique_id).and_then(|&r| self.get(r))
    }

    pub fn len(&self) -> usize {
        self.by_runtime.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_runtime.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.by_runtime.values().map(|e| &**e)
    }

    pub fn players(&self) -> impl Iterator<Item = &Entity> {
        self.iter().filter(|e| e.is_player())
    }

    /// Closest entity to `pos` (by wire position) that passes `filter`.
    pub fn nearest(&self, pos: &Vec3f, filter: impl Fn(&Entity) -> bool) -> Option<&Entity> {
        self.iter().filter(|e| filter(e)).min_by(|a, b| a.distance_sq(pos).total_cmp(&b.distance_sq(pos)))
    }

    /// The value of synced property `name` (`minecraft:climate_variant`) on `e`, as its kind's
    /// definition reads it; `None` when the kind has no such property or the server sent none.
    pub fn property(&self, e: &Entity, name: &str) -> Option<Property<'_>> {
        self.property_defs.value(&e.kind, name, &e.properties)
    }

    /// Every synced property of `e` the server has sent, by name.
    pub fn properties<'a>(&'a self, e: &'a Entity) -> impl Iterator<Item = (&'a str, Property<'a>)> {
        self.property_defs.all(&e.kind, &e.properties)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tracking_tests;
