use acacia_client::proto::types::{MetadataDictionaryItem, MetadataDictionaryItemType};

use super::*;
use crate::state::queries::test_support::{fixtures, raw};

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn add_hook(runtime_id: u64, unique_id: i64, owner: i64) -> RawPacket {
    let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    (p.runtime_id, p.unique_id, p.entity_type) = (runtime_id, unique_id, FISHING_HOOK_KIND.into());
    p.metadata = vec![MetadataDictionaryItem {
        key: MetadataDictionaryItemKey::OwnerEid,
        r#type: MetadataDictionaryItemType::Long,
        legacy_type: 0,
        value: MetadataDictionaryItemValue::Default(MetadataDictionaryItemValueDefault::Long(owner)),
    }];
    raw(&p)
}

fn hook_event(runtime_id: u64, event_id: EntityEventEventId) -> RawPacket {
    raw(&EntityEvent { runtime_entity_id: runtime_id, event_id, data: 0, fire_at_position: None })
}

fn motion(runtime_id: u64, vy: f32) -> RawPacket {
    raw(&SetEntityMotion { runtime_entity_id: runtime_id, velocity: Vec3f { x: 0.0, y: vy, z: 0.0 }, tick: 0 })
}

#[test]
fn a_sudden_dip_of_the_resting_hook_is_a_bite() {
    let mut f = Fishing::default();
    f.apply(&add_hook(6, 60, ME.runtime_entity_id as i64), &ME).unwrap();
    let bites = |f: &Fishing| f.hook.as_ref().unwrap().bites;
    // The apex of a cast on BDS: one update with vy 0, then -0.228 (seen live).
    for vy in [0.3, 0.1, 0.0, -0.228, -0.25, -0.3] {
        f.apply(&motion(6, vy), &ME).unwrap();
    }
    assert_eq!(bites(&f), 0, "a falling cast is no bite");
    for vy in [0.02, -0.01, 0.0, -0.3] {
        f.apply(&motion(6, vy), &ME).unwrap();
    }
    assert_eq!(bites(&f), 1);
}

#[test]
fn tracks_only_the_own_hook_and_its_bites() {
    let mut f = Fishing::default();
    f.apply(&add_hook(5, 50, -9), &ME).unwrap();
    assert!(f.hook.is_none(), "someone else's hook");

    f.apply(&add_hook(6, 60, ME.unique_entity_id), &ME).unwrap();
    f.apply(&hook_event(6, EntityEventEventId::FishHookTease), &ME).unwrap();
    f.apply(&hook_event(5, EntityEventEventId::FishHookHook), &ME).unwrap();
    assert_eq!(f.hook.as_ref().map(|h| h.bites), Some(0));
    f.apply(&hook_event(6, EntityEventEventId::FishHookHook), &ME).unwrap();
    assert_eq!(f.hook.as_ref().map(|h| (h.runtime_id, h.bites)), Some((6, 1)));

    f.apply(&raw(&RemoveEntity { entity_id_self: 60 }), &ME).unwrap();
    assert!(f.hook.is_none());
}

#[test]
fn the_own_pickup_of_fishing_loot_is_the_catch() {
    let mut f = Fishing::default();
    let mut loot: AddItemEntity = fixtures::<AddItemEntity>()[0].decode().unwrap();
    (loot.runtime_entity_id, loot.entity_id_self, loot.is_from_fishing) = (20, 200, true);
    f.apply(&raw(&loot), &ME).unwrap();
    f.apply(&raw(&TakeItemEntity { runtime_entity_id: 20, target: 9 }), &ME).unwrap();
    assert_eq!(f.catch, None, "someone else's catch");

    (loot.runtime_entity_id, loot.entity_id_self) = (21, 201);
    f.apply(&raw(&loot), &ME).unwrap();
    f.apply(&raw(&TakeItemEntity { runtime_entity_id: 21, target: ME.runtime_entity_id as u32 }), &ME).unwrap();
    assert_eq!(f.catch, Some(ItemStack::from(loot.item.clone())));
}
