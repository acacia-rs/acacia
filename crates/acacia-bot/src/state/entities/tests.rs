use acacia_client::proto::packets::MovePlayerMode;
use acacia_client::proto::types::Rotation;

use super::*;
use crate::state::queries::test_support::{fixtures, raw};

const ME: Me = Me { runtime_entity_id: 1, unique_entity_id: -1 };

fn fixture<T: Packet>() -> T {
    fixtures::<T>()[0].decode().unwrap()
}

fn v(x: f32, y: f32, z: f32) -> Vec3f {
    Vec3f { x, y, z }
}

fn add_player(runtime_id: u64, unique_id: i64, name: &str, pos: Vec3f) -> RawPacket {
    let mut p: AddPlayer = fixture();
    (p.runtime_id, p.unique_id, p.username, p.position) = (runtime_id, unique_id, name.into(), pos);
    (p.yaw, p.pitch, p.head_yaw) = (90.0, 10.0, 95.0);
    raw(&p)
}

fn add_entity(runtime_id: u64, unique_id: i64, kind: &str, pos: Vec3f) -> RawPacket {
    let mut p: AddEntity = fixture();
    (p.runtime_id, p.unique_id, p.entity_type, p.position) = (runtime_id, unique_id, kind.into(), pos);
    raw(&p)
}

fn add_item(runtime_id: u64, unique_id: i64, pos: Vec3f) -> RawPacket {
    let mut p: AddItemEntity = fixture();
    (p.runtime_entity_id, p.entity_id_self, p.position) = (runtime_id, unique_id, pos);
    raw(&p)
}

fn delta(runtime_id: u64) -> MoveEntityDelta {
    MoveEntityDelta {
        runtime_entity_id: runtime_id,
        x: None,
        y: None,
        z: None,
        rot_x: None,
        rot_y: None,
        rot_z: None,
        on_ground: false,
        force_move: false,
        force_move_local_entity: false,
        force_completion: false,
        ticks: 0,
    }
}

fn world() -> Entities {
    let mut es = Entities::default();
    es.apply(&add_player(1, -1, "Me", v(0.0, 0.0, 0.0)), &ME).unwrap();
    es.apply(&add_player(2, -2, "Steve", v(10.0, 65.62, 0.0)), &ME).unwrap();
    es.apply(&add_entity(3, -3, "minecraft:zombie", v(2.0, 64.0, 0.0)), &ME).unwrap();
    es.apply(&add_item(4, -4, v(-5.0, 64.0, 0.0)), &ME).unwrap();
    es
}

#[test]
fn spawns_are_tracked_without_local_player() {
    let es = world();
    assert_eq!(es.len(), 3);
    assert!(es.get(1).is_none());

    let steve = es.get(2).unwrap();
    assert_eq!((steve.kind.as_str(), steve.username.as_deref()), (PLAYER_KIND, Some("Steve")));
    assert_eq!((steve.yaw, steve.pitch, steve.head_yaw), (90.0, 10.0, 95.0));
    assert!((steve.feet().y - 64.0).abs() < 1e-4);

    assert_eq!(es.get(3).unwrap().kind, "minecraft:zombie");
    assert_eq!(es.get(4).unwrap().kind, ITEM_KIND);
    assert_eq!(es.by_unique(-3).unwrap().runtime_id, 3);
    assert_eq!(es.players().map(|e| e.runtime_id).collect::<Vec<_>>(), [2]);
}

#[test]
fn metadata_updates_reach_tracked_entities_only() {
    use acacia_client::proto::types::{
        MetadataDictionaryItem, MetadataDictionaryItemKey, MetadataDictionaryItemType, MetadataDictionaryItemValue as Value,
        MetadataFlags1,
    };
    let mut es = world();
    assert!(es.get(2).unwrap().uuid.is_some() && es.get(3).unwrap().uuid.is_none());
    let mut p: SetEntityData = fixture();
    p.metadata = vec![MetadataDictionaryItem {
        key: MetadataDictionaryItemKey::Flags,
        r#type: MetadataDictionaryItemType::Long,
        legacy_type: 7,
        value: Value::Flags(MetadataFlags1::BABY),
    }];
    p.runtime_entity_id = 3;
    es.apply(&raw(&p), &ME).unwrap();
    assert!(es.get(3).unwrap().meta.is_baby());
    assert_eq!(es.get(3).unwrap().meta.scale, 1.0);
    p.runtime_entity_id = 99;
    es.apply(&raw(&p), &ME).unwrap();
    assert!(!es.get(2).unwrap().meta.is_baby());
}

#[test]
fn remove_uses_unique_id() {
    let mut es = world();
    es.apply(&raw(&RemoveEntity { entity_id_self: -3 }), &ME).unwrap();
    assert!(es.get(3).is_none());
    assert!(es.by_unique(-3).is_none());
    assert_eq!(es.len(), 2);
}

#[test]
fn move_delta_updates_only_present_fields() {
    let mut es = world();
    let pitch = es.get(3).unwrap().pitch;
    let mut p = delta(3);
    (p.x, p.rot_y, p.rot_z, p.on_ground) = (Some(7.5), Some(64), Some(128), true);
    es.apply(&raw(&p), &ME).unwrap();
    let z = es.get(3).unwrap();
    assert_eq!(z.position, v(7.5, 64.0, 0.0));
    assert_eq!((z.pitch, z.yaw, z.head_yaw, z.on_ground), (pitch, 90.0, 180.0, true));
}

#[test]
fn move_absolute_sets_position_rotation_and_ground() {
    let mut es = world();
    let rotation = Rotation { yaw: 45.0, pitch: 90.0, head_yaw: 180.0 };
    es.apply(&raw(&MoveEntity { runtime_entity_id: 3, flags: 1, position: v(1.0, 2.0, 3.0), rotation }), &ME).unwrap();
    let z = es.get(3).unwrap();
    assert_eq!(z.position, v(1.0, 2.0, 3.0));
    assert_eq!((z.pitch, z.yaw, z.head_yaw, z.on_ground), (45.0, 90.0, 180.0, true));
}

#[test]
fn move_player_and_motion() {
    let mut es = world();
    es.apply(
        &raw(&MovePlayer {
            runtime_id: 2,
            position: v(11.0, 66.62, 1.0),
            pitch: -5.0,
            yaw: 180.0,
            head_yaw: 170.0,
            mode: MovePlayerMode::Normal,
            on_ground: true,
            ridden_runtime_id: 0,
            teleport: None,
            tick: 0,
        }),
        &ME,
    )
    .unwrap();
    es.apply(&raw(&SetEntityMotion { runtime_entity_id: 2, velocity: v(0.0, 0.42, 0.0), tick: 0 }), &ME).unwrap();
    let s = es.get(2).unwrap();
    assert_eq!(s.position, v(11.0, 66.62, 1.0));
    assert_eq!((s.pitch, s.yaw, s.head_yaw, s.on_ground), (-5.0, 180.0, 170.0, true));
    assert_eq!(s.velocity, v(0.0, 0.42, 0.0));
}

#[test]
fn nearest_respects_filter() {
    let es = world();
    let origin = v(0.0, 64.0, 0.0);
    assert_eq!(es.nearest(&origin, |_| true).unwrap().runtime_id, 3);
    assert_eq!(es.nearest(&origin, Entity::is_player).unwrap().runtime_id, 2);
    assert!(es.nearest(&origin, |e| e.kind == "minecraft:creeper").is_none());
}

#[test]
fn respawned_runtime_id_drops_stale_unique_index() {
    let mut es = world();
    es.apply(&add_entity(3, -30, "minecraft:skeleton", v(0.0, 0.0, 0.0)), &ME).unwrap();
    assert!(es.by_unique(-3).is_none());
    assert_eq!(es.by_unique(-30).unwrap().kind, "minecraft:skeleton");
}

#[test]
fn dimension_change_clears() {
    let mut es = world();
    es.apply(&fixtures::<ChangeDimension>()[0], &ME).unwrap();
    assert!(es.is_empty());
    assert!(es.by_unique(-2).is_none());
}
