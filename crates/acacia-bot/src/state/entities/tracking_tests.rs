use acacia_client::proto::packets::{AddEntity, MoveEntity, MoveEntityDelta, SetEntityData, SetEntityMotion, UpdateAttributes};
use acacia_client::proto::types::{
    MetadataDictionaryItem, MetadataDictionaryItemKey as Key, MetadataDictionaryItemType as Type, MetadataDictionaryItemValue as Value,
    MetadataDictionaryItemValueDefault as Plain, Rotation, Vec3f,
};

use super::Entities;
use crate::state::queries::test_support::{fixtures, raw};
use crate::state::Me;

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn v(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f { x, y, z }
}

fn zombie(es: &mut Entities) {
    let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    (p.runtime_id, p.unique_id, p.entity_type, p.position, p.velocity) = (3, -3, "minecraft:zombie".into(), v(0.0, 64.0, 0.0), v(0.0, 0.0, 0.0));
    es.apply(&raw(&p), &ME).unwrap();
}

fn step_x(es: &mut Entities, x: f32) {
    let p = MoveEntityDelta {
        runtime_entity_id: 3,
        x: Some(x),
        y: None,
        z: None,
        rot_x: None,
        rot_y: None,
        rot_z: None,
        on_ground: true,
        force_move: false,
        force_move_local_entity: false,
        force_completion: false,
        ticks: 0,
    };
    es.apply(&raw(&p), &ME).unwrap();
}

#[test]
fn motion_is_derived_over_the_ticks_between_moves() {
    let mut es = Entities::default();
    zombie(&mut es);
    es.advance();
    step_x(&mut es, 0.25);
    assert_eq!(es.get(3).unwrap().motion, v(0.25, 0.0, 0.0));
    // BDS paces far mobs' moves: a gap of four ticks spreads the step over them.
    (0..4).for_each(|_| es.advance());
    step_x(&mut es, 1.25);
    let e = es.get(3).unwrap();
    assert_eq!((e.motion.clone(), e.moved_at), (v(0.25, 0.0, 0.0), 5));
}

#[test]
fn told_motion_replaces_derived_until_the_next_move() {
    let mut es = Entities::default();
    zombie(&mut es);
    es.advance();
    es.apply(&raw(&SetEntityMotion { runtime_entity_id: 3, velocity: v(0.0, 0.42, 0.0), tick: 0 }), &ME).unwrap();
    assert_eq!(es.get(3).unwrap().motion, v(0.0, 0.42, 0.0));
    step_x(&mut es, 0.1);
    assert_eq!(es.get(3).unwrap().motion, v(0.1, 0.0, 0.0));
}

#[test]
fn teleports_carry_no_motion() {
    let mut es = Entities::default();
    zombie(&mut es);
    es.advance();
    let rotation = Rotation { yaw: 0.0, pitch: 0.0, head_yaw: 0.0 };
    es.apply(&raw(&MoveEntity { runtime_entity_id: 3, flags: 2, position: v(40.0, 64.0, 0.0), rotation }), &ME).unwrap();
    assert_eq!(es.get(3).unwrap().motion, v(0.0, 0.0, 0.0));
}

#[test]
fn lean_tracking_keeps_only_hazard_metadata() {
    let mut es = Entities { lean: true, ..Entities::default() };
    zombie(&mut es);
    let item = |key, r#type, value| MetadataDictionaryItem { key, r#type, legacy_type: 0, value: Value::Default(value) };
    let mut data: SetEntityData = fixtures::<SetEntityData>()[0].decode().unwrap();
    data.runtime_entity_id = 3;
    data.metadata = vec![item(Key::Nametag, Type::String, Plain::String("Bob".into())), item(Key::Scale, Type::Float, Plain::Float(2.0))];
    es.apply(&raw(&data), &ME).unwrap();
    let mut attrs: UpdateAttributes = fixtures::<UpdateAttributes>()[0].decode().unwrap();
    attrs.runtime_entity_id = 3;
    es.apply(&raw(&attrs), &ME).unwrap();
    let e = es.get(3).unwrap();
    assert_eq!((e.metadata.name_tag(), e.metadata.scale()), (None, 2.0));
    assert!(e.attributes.is_empty());
}
