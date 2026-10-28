use acacia_client::proto::packets::AddEntity;
use acacia_client::proto::types::{
    MetadataDictionaryItem, MetadataDictionaryItemKey as Key, MetadataDictionaryItemType as Type, MetadataDictionaryItemValue as Value,
    MetadataDictionaryItemValueDefault as Plain, MetadataFlags1, Vec3f,
};

use super::*;
use crate::state::queries::test_support::{fixtures, raw};
use crate::state::Me;

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn spawn(es: &mut Entities, id: u64, kind: &str, at: [f32; 3], metadata: Vec<MetadataDictionaryItem>) {
    let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    let [x, y, z] = at;
    (p.runtime_id, p.unique_id, p.entity_type, p.position) = (id, -(id as i64), kind.into(), Vec3f { x, y, z });
    p.metadata = metadata;
    es.apply(&raw(&p), &ME).unwrap();
}

fn flags(f: MetadataFlags1) -> MetadataDictionaryItem {
    MetadataDictionaryItem { key: Key::Flags, r#type: Type::Long, legacy_type: 7, value: Value::Flags(f) }
}

fn float(key: Key, v: f32) -> MetadataDictionaryItem {
    MetadataDictionaryItem { key, r#type: Type::Float, legacy_type: 3, value: Value::Default(Plain::Float(v)) }
}

fn threat(es: &Entities, id: u64) -> Threat {
    Threat::of(es.get(id).unwrap(), es.now()).unwrap()
}

#[test]
fn classes_and_boxes_by_kind() {
    let mut es = Entities::default();
    spawn(&mut es, 2, "minecraft:zombie", [0.5, 64.0, 0.5], vec![float(Key::BoundingboxWidth, 0.6), float(Key::BoundingboxHeight, 1.9)]);
    spawn(&mut es, 3, "minecraft:boat", [4.5, 64.375, 0.5], vec![]);
    spawn(&mut es, 4, "minecraft:cow", [8.5, 64.0, 0.5], vec![]);
    spawn(&mut es, 5, "minecraft:xp_orb", [9.5, 64.0, 0.5], vec![]);
    let z = threat(&es, 2);
    assert_eq!((z.class, z.size, z.hostile()), (Class::Melee, (0.6, 1.9), true));
    let boat = threat(&es, 3);
    // The boat's wire y is its box bottom + 0.375.
    assert_eq!((boat.class, boat.feet, boat.size), (Class::Solid, [4.5, 64.0, 0.5], (1.4, 0.455)));
    assert_eq!((threat(&es, 4).class, threat(&es, 4).hostile()), (Class::Passive, false));
    assert!(Threat::of(es.get(5).unwrap(), 0).is_none());
    assert_eq!(Threats::near(&es, [0.5, 64.0, 0.5], 5.0).list.len(), 2);
}

#[test]
fn lit_creeper_and_angry_neutral_are_flagged() {
    let mut es = Entities::default();
    spawn(&mut es, 2, "minecraft:creeper", [0.5, 64.0, 0.5], vec![flags(MetadataFlags1::IGNITED)]);
    spawn(&mut es, 3, "minecraft:wolf", [2.5, 64.0, 0.5], vec![flags(MetadataFlags1::ANGRY)]);
    spawn(&mut es, 4, "minecraft:enderman", [4.5, 64.0, 0.5], vec![]);
    assert!(threat(&es, 2).lit);
    assert!(threat(&es, 3).angry && threat(&es, 3).hostile());
    assert!(!threat(&es, 4).hostile());
}

#[test]
fn age_counts_ticks_since_the_last_move() {
    let mut es = Entities::default();
    spawn(&mut es, 2, "minecraft:zombie", [0.5, 64.0, 0.5], vec![]);
    (0..3).for_each(|_| es.advance());
    assert_eq!(threat(&es, 2).age, 3);
}
