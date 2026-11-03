use rustc_hash::FxHashMap;

use super::delta::Changes;
use super::entity::{same_members, EntityInfo, EntityList, Interner};
use crate::state::Entities;

/// Builds the published [`EntityList`]s: gives each entity its slot and says what changed since the last list.
#[derive(Default)]
pub(crate) struct ListBuilder {
    interner: Interner,
    /// Slots freed by removals, reused before new ones.
    free: Vec<u32>,
    capacity: u32,
}

impl ListBuilder {
    /// `previous` is the list this one replaces (the last this builder built, or an empty one).
    pub(crate) fn build(&mut self, entities: &Entities, version: u64, previous: &EntityList) -> (EntityList, Changes) {
        let infos: Vec<EntityInfo> = entities.iter().map(|e| EntityInfo::of(e, &mut |s| self.interner.get(s))).collect();
        let mut changes = Changes::default();
        let same = same_members(&previous.entities, &infos);
        let slots = match same {
            true => {
                let moved = previous.entities.iter().zip(&infos).zip(&previous.slots).filter(|((a, b), _)| !a.same_numbers(b));
                changes.moved.extend(moved.map(|(_, &slot)| slot));
                previous.slots.clone()
            }
            false => self.reassign(previous, &infos, &mut changes),
        };
        let members_version = previous.members_version + u64::from(!same);
        (EntityList::new(version, members_version, infos, slots, self.capacity), changes)
    }

    /// Slots for a membership change: kept for entities that stayed, freed for those gone, then handed out.
    fn reassign(&mut self, previous: &EntityList, infos: &[EntityInfo], changes: &mut Changes) -> Vec<u32> {
        let mut gone: FxHashMap<u64, usize> = previous.entities.iter().enumerate().map(|(i, e)| (e.runtime_id, i)).collect();
        let mut slots = vec![u32::MAX; infos.len()];
        for (new, slot) in infos.iter().zip(&mut slots) {
            let Some(&i) = gone.get(&new.runtime_id).filter(|&&i| previous.entities[i].same_member(new)) else { continue };
            gone.remove(&new.runtime_id);
            *slot = previous.slots[i];
            if !previous.entities[i].same_numbers(new) {
                changes.moved.push(*slot);
            }
        }
        let mut gone: Vec<usize> = gone.into_values().collect();
        gone.sort_unstable();
        for i in gone {
            changes.removed.push((previous.slots[i], previous.entities[i].clone()));
            self.free.push(previous.slots[i]);
        }
        for slot in slots.iter_mut().filter(|s| **s == u32::MAX) {
            *slot = self.free.pop().unwrap_or_else(|| {
                self.capacity += 1;
                self.capacity - 1
            });
            changes.added.push(*slot);
        }
        slots
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use acacia_client::proto::packets::{AddEntity, MoveEntity, RemoveEntity};
    use acacia_client::proto::types::{Rotation, Vec3f};

    use super::*;
    use crate::state::queries::test_support::{fixtures, raw};
    use crate::state::Me;

    const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

    thread_local!(static ADD: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap());

    pub(crate) fn add(es: &mut Entities, id: u64, x: f32) {
        let mut p = ADD.with(AddEntity::clone);
        let position = Vec3f { x, y: 64.0, z: 0.0 };
        (p.runtime_id, p.unique_id, p.entity_type, p.position) = (id, -(id as i64), "minecraft:zombie".into(), position);
        es.apply(&raw(&p), &ME).unwrap();
    }

    pub(crate) fn walk(es: &mut Entities, id: u64, x: f32) {
        let rotation = Rotation { yaw: 0.0, pitch: 0.0, head_yaw: 0.0 };
        es.apply(&raw(&MoveEntity { runtime_entity_id: id, flags: 0, position: Vec3f { x, y: 64.0, z: 0.0 }, rotation }), &ME).unwrap();
    }

    pub(crate) fn remove(es: &mut Entities, id: u64) {
        es.apply(&raw(&RemoveEntity { entity_id_self: -(id as i64) }), &ME).unwrap();
    }

    fn slot_of(list: &EntityList, id: u64) -> u32 {
        list.slots[list.entities.iter().position(|e| e.runtime_id == id).unwrap()]
    }

    #[test]
    fn slots_stay_with_their_entity_and_are_reused_after_it_left() {
        let (mut es, mut b) = (Entities::default(), ListBuilder::default());
        add(&mut es, 3, 0.0);
        add(&mut es, 4, 1.0);
        let (first, changes) = b.build(&es, 1, &EntityList::default());
        assert_eq!((changes.added.len(), first.slot_capacity()), (2, 2));
        walk(&mut es, 3, 5.0);
        let (moved, changes) = b.build(&es, 2, &first);
        assert_eq!(changes.moved, [slot_of(&first, 3)]);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        remove(&mut es, 3);
        add(&mut es, 5, 2.0);
        let (swapped, changes) = b.build(&es, 3, &moved);
        assert_eq!(changes.removed.iter().map(|(s, e)| (*s, e.runtime_id, e.position[0])).collect::<Vec<_>>(), [(slot_of(&first, 3), 3, 5.0)]);
        assert_eq!(changes.added, [slot_of(&first, 3)], "the freed slot is reused");
        assert_eq!((slot_of(&swapped, 5), slot_of(&swapped, 4)), (slot_of(&first, 3), slot_of(&first, 4)));
        assert_eq!(swapped.index_of_slot(slot_of(&swapped, 5)).map(|i| swapped.entities[i].runtime_id), Some(5));
        assert_eq!([first.members_version, moved.members_version, swapped.members_version], [1, 1, 2]);
    }
}
