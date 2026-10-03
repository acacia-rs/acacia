use acacia_client::proto::types::Vec3f;

use super::*;
use crate::state::queries::test_support::{fixtures, raw};

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn add_entity(runtime_id: u64, unique_id: i64, kind: &str, links: Vec<Link>) -> RawPacket {
    let mut p: AddEntity = fixtures::<AddEntity>()[0].decode().unwrap();
    (p.runtime_id, p.unique_id, p.entity_type, p.links) = (runtime_id, unique_id, kind.into(), links);
    p.position = Vec3f { x: 0.0, y: 64.0, z: 0.0 };
    raw(&p)
}

fn link(ridden: i64, rider: i64, r#type: u8) -> Link {
    Link { ridden_entity_id: ridden, rider_entity_id: rider, r#type, immediate: false, rider_initiated: true, angular_velocity: 0.0 }
}

fn set_link(ridden: i64, rider: i64, r#type: u8) -> RawPacket {
    raw(&SetEntityLink { link: link(ridden, rider, r#type) })
}

#[test]
fn mount_and_dismount_from_links() {
    let mut r = Riding::default();
    r.apply(&add_entity(7, 70, "minecraft:boat", vec![]), &ME).unwrap();
    r.apply(&set_link(70, -5, LINK_RIDER), &ME).unwrap();
    assert!(!r.is_riding(), "another entity's link");

    r.apply(&set_link(70, ME.unique_entity_id, 2), &ME).unwrap();
    let v = r.vehicle.clone().unwrap();
    assert_eq!((v.unique_id, v.runtime_id, v.kind.as_deref(), v.driver), (70, Some(7), Some("minecraft:boat"), false));

    r.apply(&set_link(70, ME.unique_entity_id, LINK_REMOVE), &ME).unwrap();
    assert!(!r.is_riding());
}

#[test]
fn seated_by_spawn_links_and_unknown_vehicles() {
    let mut r = Riding::default();
    r.apply(&add_entity(9, 90, "minecraft:minecart", vec![link(90, ME.unique_entity_id, LINK_RIDER)]), &ME).unwrap();
    assert_eq!(r.vehicle.as_ref().map(|v| (v.runtime_id, v.driver)), Some((Some(9), true)));

    r.apply(&raw(&RemoveEntity { entity_id_self: 90 }), &ME).unwrap();
    assert!(!r.is_riding(), "the vehicle despawned");
    assert_eq!(r.runtime_id_of(90), None);

    r.apply(&set_link(123, ME.unique_entity_id, LINK_RIDER), &ME).unwrap();
    assert_eq!(r.vehicle.as_ref().map(|v| (v.runtime_id, v.kind.clone())), Some((None, None)));
    r.apply(&set_link(124, ME.unique_entity_id, LINK_REMOVE), &ME).unwrap();
    assert!(r.is_riding(), "removing a link to another vehicle keeps the seat");
}
