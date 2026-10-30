use std::collections::HashSet;
use std::sync::Arc;

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
        let f = e.feet();
        let v = &e.velocity;
        EntityInfo {
            runtime_id: e.runtime_id,
            unique_id: e.unique_id,
            kind: intern(&e.kind),
            username: e.username.as_deref().map(&mut *intern),
            position: [f.x, f.y, f.z],
            velocity: [v.x, v.y, v.z],
            yaw: e.yaw,
            pitch: e.pitch,
            health: e.health(),
        }
    }

    /// For events: allocates its strings.
    pub(crate) fn copy(e: &Entity) -> Self {
        Self::of(e, &mut |s: &str| Arc::from(s))
    }
}

/// Every tracked entity, rebuilt at most once a tick when the tracker changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntityList {
    /// Moves exactly when the list is rebuilt (0 in an on-demand snapshot).
    pub version: u64,
    pub entities: Vec<EntityInfo>,
}

/// Kind and player-name strings the published lists share.
#[derive(Default)]
pub(crate) struct Interner(HashSet<Arc<str>>);

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

pub(crate) fn build_list(entities: &crate::state::Entities, version: u64, intern: &mut Interner) -> EntityList {
    EntityList { version, entities: entities.iter().map(|e| EntityInfo::of(e, &mut |s| intern.get(s))).collect() }
}
