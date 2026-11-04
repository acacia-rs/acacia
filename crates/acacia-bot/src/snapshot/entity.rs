use std::sync::Arc;

use rustc_hash::FxHashSet;

use crate::state::Entity;

/// A tracked entity or other player, summarised: what events and [`EntityList`] carry.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityInfo {
    pub runtime_id: u64,
    pub unique_id: i64,
    /// `minecraft:zombie`; `minecraft:player` for players.
    pub kind: Arc<str>,
    /// Players only.
    pub username: Option<Arc<str>>,
    /// Feet, also for players (see [`Entity::feet`]).
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// `minecraft:health` when the server sent it (mobs only).
    pub health: Option<f32>,
}

impl EntityInfo {
    /// `intern` shares the kind and name strings between rebuilds.
    pub(crate) fn of(e: &Entity, intern: &mut impl FnMut(&str) -> Arc<str>) -> Self {
        let (position, velocity, yaw, pitch, health) = numbers(e);
        EntityInfo {
            runtime_id: e.runtime_id,
            unique_id: e.unique_id,
            kind: intern(&e.kind),
            username: e.username.as_deref().map(&mut *intern),
            position,
            velocity,
            yaw,
            pitch,
            health,
        }
    }

    /// Takes `e`'s position, motion, rotation and health; true when any of them changed.
    pub(crate) fn update_numbers(&mut self, e: &Entity) -> bool {
        let fresh = numbers(e);
        let held = (self.position, self.velocity, self.yaw, self.pitch, self.health);
        if fresh == held {
            return false;
        }
        (self.position, self.velocity, self.yaw, self.pitch, self.health) = fresh;
        true
    }

    /// For events: allocates its strings.
    pub(crate) fn copy(e: &Entity) -> Self {
        Self::of(e, &mut |s: &str| Arc::from(s))
    }

    /// Its kind or username when it equals `s`.
    pub(crate) fn string(&self, s: &str) -> Option<Arc<str>> {
        std::iter::once(&self.kind).chain(&self.username).find(|k| ***k == *s).cloned()
    }

    /// The same entity: ids, kind and name match (its numbers may differ).
    pub fn same_member(&self, other: &EntityInfo) -> bool {
        (self.runtime_id, self.unique_id) == (other.runtime_id, other.unique_id) && self.kind == other.kind && self.username == other.username
    }

    /// Position, motion, rotation and health match.
    pub fn same_numbers(&self, other: &EntityInfo) -> bool {
        (self.position, self.velocity, self.yaw, self.pitch, self.health) == (other.position, other.velocity, other.yaw, other.pitch, other.health)
    }
}

type Numbers = ([f32; 3], [f32; 3], f32, f32, Option<f32>);

fn numbers(e: &Entity) -> Numbers {
    let (f, v) = (e.feet(), &e.velocity);
    ([f.x, f.y, f.z], [v.x, v.y, v.z], e.yaw, e.pitch, e.health())
}

/// Every tracked entity, updated at most once a tick when the tracker changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntityList {
    /// Moves exactly when the list changes (0 in an on-demand snapshot).
    pub version: u64,
    /// Moves when the ids, kinds or names change (an entity came or went), not when entities only move or turn.
    pub members_version: u64,
    pub entities: Vec<EntityInfo>,
    /// `entities[i]`'s slot: kept while it stays listed, reused once it left (see [`super::delta`]).
    pub slots: Vec<u32>,
    /// Index into `entities` per slot, [`EntityList::FREE`] for an unused one.
    by_slot: Vec<u32>,
}

impl EntityList {
    const FREE: u32 = u32::MAX;

    /// `capacity` bounds the slots: every one in `slots` is below it.
    pub(crate) fn new(version: u64, members_version: u64, entities: Vec<EntityInfo>, slots: Vec<u32>, capacity: u32) -> Self {
        let mut by_slot = vec![Self::FREE; capacity as usize];
        for (i, &slot) in slots.iter().enumerate() {
            by_slot[slot as usize] = i as u32;
        }
        EntityList { version, members_version, entities, slots, by_slot }
    }

    /// The same entities in the same order: only positions, motion, rotation or health may differ.
    pub fn same_members(&self, other: &EntityList) -> bool {
        same_members(&self.entities, &other.entities)
    }

    /// Every slot in use is below this.
    pub fn slot_capacity(&self) -> u32 {
        self.by_slot.len() as u32
    }

    /// The index into `entities` of the entity in `slot`.
    pub fn index_of_slot(&self, slot: u32) -> Option<usize> {
        self.by_slot.get(slot as usize).filter(|&&i| i != Self::FREE).map(|&i| i as usize)
    }
}

pub(crate) fn same_members(a: &[EntityInfo], b: &[EntityInfo]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.same_member(b))
}

/// Kind and player-name strings the published lists share.
#[derive(Default)]
pub(crate) struct Interner(FxHashSet<Arc<str>>);

impl Interner {
    /// Bounds the set on servers that churn through many names.
    const MAX: usize = 4096;

    pub(crate) fn get(&mut self, s: &str) -> Arc<str> {
        if let Some(s) = self.0.get(s) {
            return s.clone();
        }
        if self.0.len() >= Self::MAX {
            self.0.clear();
        }
        let s: Arc<str> = s.into();
        self.0.insert(s.clone());
        s
    }
}
