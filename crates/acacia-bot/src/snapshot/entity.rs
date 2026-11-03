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
    /// Moves when the ids, kinds or names change (an entity came or went), not when entities only move or turn.
    pub members_version: u64,
    pub entities: Vec<EntityInfo>,
}

impl EntityList {
    /// The same entities in the same order: only positions, motion, rotation or health may differ.
    pub fn same_members(&self, other: &EntityList) -> bool {
        self.entities.len() == other.entities.len()
            && self.entities.iter().zip(&other.entities).all(|(a, b)| {
                (a.runtime_id, a.unique_id) == (b.runtime_id, b.unique_id) && a.kind == b.kind && a.username == b.username
            })
    }
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

/// `previous` is the list this one replaces, for [`EntityList::members_version`].
pub(crate) fn build_list(entities: &crate::state::Entities, version: u64, previous: &EntityList, intern: &mut Interner) -> EntityList {
    let entities = entities.iter().map(|e| EntityInfo::of(e, &mut |s| intern.get(s))).collect();
    let mut list = EntityList { version, members_version: previous.members_version, entities };
    if !list.same_members(previous) {
        list.members_version += 1;
    }
    list
}

#[cfg(test)]
mod tests {
    use acacia_client::proto::packets::{AddEntity, MoveEntity, RemoveEntity};
    use acacia_client::proto::types::{Rotation, Vec3f};

    use super::*;
    use crate::state::queries::test_support::{fixtures, raw};
    use crate::state::{Entities, Me};

    const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

    #[test]
    fn members_version_moves_on_spawn_and_removal_not_on_moves() {
        let (mut es, mut intern) = (Entities::default(), Interner::default());
        let position = |x| Vec3f { x, y: 64.0, z: 0.0 };
        let add = |es: &mut Entities, id: u64| {
            let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
            (p.runtime_id, p.unique_id, p.entity_type, p.position) = (id, -(id as i64), "minecraft:zombie".into(), position(0.0));
            es.apply(&raw(&p), &ME).unwrap();
        };
        add(&mut es, 3);
        let first = build_list(&es, 1, &EntityList::default(), &mut intern);
        let rotation = Rotation { yaw: 0.0, pitch: 0.0, head_yaw: 0.0 };
        es.apply(&raw(&MoveEntity { runtime_entity_id: 3, flags: 0, position: position(5.0), rotation }), &ME).unwrap();
        let moved = build_list(&es, 2, &first, &mut intern);
        add(&mut es, 4);
        let grown = build_list(&es, 3, &moved, &mut intern);
        es.apply(&raw(&RemoveEntity { entity_id_self: -4 }), &ME).unwrap();
        let shrunk = build_list(&es, 4, &grown, &mut intern);

        assert_eq!(moved.entities[0].position[0], 5.0);
        assert_eq!([first.members_version, moved.members_version, grown.members_version, shrunk.members_version], [1, 1, 2, 3]);
    }
}
