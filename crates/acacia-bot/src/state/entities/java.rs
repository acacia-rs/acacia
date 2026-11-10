//! The Java client's entities as tracked [`Entity`]s, synced once a tick: kinds in Bedrock's spelling (what
//! hazards and callers compare), players at eye height like Bedrock's wire positions, `motion` from the last move.
//! Java names no mob's target, so an aggressive mob is taken to target the bot (`TargetEid`).

use std::collections::HashMap;

use acacia_client::proto::manual::Uuid;
use acacia_client::proto::types::{MetadataDictionaryItemKey as Key, MetadataDictionaryItemValue as Value, MetadataDictionaryItemValueDefault as Plain, Vec3f};
use acacia_java_client::world::Entity as JavaEntity;
use acacia_java_proto::entity_types::ENTITY_TYPES;

use super::{Entities, Entity, PLAYER_EYE_HEIGHT, PLAYER_KIND};

impl Entities {
    pub(crate) fn mirror_java(&mut self, java: &HashMap<i32, JavaEntity>, me: i64) {
        self.by_runtime.retain(|id, _| i32::try_from(*id).is_ok_and(|id| java.contains_key(&id)));
        self.runtime_by_unique.retain(|_, runtime| self.by_runtime.contains_key(runtime));
        for (&id, e) in java {
            let kind = bedrock_kind(e.kind);
            let lift = if kind == PLAYER_KIND { PLAYER_EYE_HEIGHT } else { 0.0 };
            let [x, y, z] = e.position.map(|v| v as f32);
            let position = Vec3f { x, y: y + lift, z };
            let runtime = id as u64;
            match self.by_runtime.get_mut(&runtime) {
                Some(t) => {
                    if t.position != position {
                        let p = &t.position;
                        t.motion = Vec3f { x: position.x - p.x, y: position.y - p.y, z: position.z - p.z };
                        t.position = position;
                        t.moved_at = self.now;
                    }
                    (t.yaw, t.pitch, t.head_yaw, t.on_ground) = (e.y_rot, e.x_rot, e.y_rot, e.on_ground);
                    t.metadata.set(Key::TargetEid, Value::Default(Plain::Long(if e.aggressive { me } else { -1 })));
                }
                None => {
                    let mut t = Entity::spawned(runtime, i64::from(id), kind.into(), position, Vec3f { x: 0.0, y: 0.0, z: 0.0 });
                    (t.yaw, t.pitch, t.head_yaw, t.moved_at) = (e.y_rot, e.x_rot, e.y_rot, self.now);
                    t.uuid = (kind == PLAYER_KIND).then(|| Uuid(e.uuid.to_be_bytes()));
                    self.runtime_by_unique.insert(i64::from(id), runtime);
                    self.by_runtime.insert(runtime, Box::new(t));
                }
            }
        }
    }
}

/// Java's entity type name as Bedrock spells it, where the two differ for something a bot reacts to.
pub(crate) fn bedrock_kind(kind: i32) -> &'static str {
    let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("minecraft:unknown");
    match name {
        n if n.ends_with("_chest_boat") || n.ends_with("_chest_raft") => "minecraft:chest_boat",
        n if n.ends_with("_boat") || n.ends_with("_raft") => "minecraft:boat",
        "minecraft:trident" => "minecraft:thrown_trident",
        "minecraft:zombified_piglin" => "minecraft:zombie_pigman",
        "minecraft:experience_orb" => "minecraft:xp_orb",
        "minecraft:experience_bottle" => "minecraft:xp_bottle",
        "minecraft:end_crystal" => "minecraft:ender_crystal",
        "minecraft:eye_of_ender" => "minecraft:eye_of_ender_signal",
        "minecraft:firework_rocket" => "minecraft:fireworks_rocket",
        "minecraft:fishing_bobber" => "minecraft:fishing_hook",
        n => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(kind: &str, position: [f64; 3]) -> JavaEntity {
        let kind = ENTITY_TYPES.iter().position(|n| *n == kind).unwrap() as i32;
        JavaEntity::new(7, kind, position, 90.0, 0.0)
    }

    #[test]
    fn java_entities_track_moves_and_removals_in_bedrock_terms() {
        let mut tracked = Entities::default();
        let mut java = HashMap::from([(3, entity("minecraft:zombie", [1.0, 64.0, 2.0])), (4, entity("minecraft:player", [0.0, 64.0, 0.0]))]);
        tracked.mirror_java(&java, 9);
        assert_eq!(tracked.get(3).map(|e| e.kind.as_str()), Some("minecraft:zombie"));
        assert_eq!(tracked.get(4).map(|e| (e.feet().y, e.position.y)), Some((64.0, 64.0 + PLAYER_EYE_HEIGHT)));
        tracked.advance();
        java.get_mut(&3).unwrap().position = [1.5, 64.0, 2.0];
        java.remove(&4);
        java.get_mut(&3).unwrap().aggressive = true;
        tracked.mirror_java(&java, 9);
        let zombie = tracked.get(3).unwrap();
        assert_eq!((zombie.motion.x, zombie.moved_at, tracked.len()), (0.5, 1, 1));
        assert_eq!(zombie.metadata.entity(Key::TargetEid), Some(9));
        assert_eq!(bedrock_kind(ENTITY_TYPES.iter().position(|n| *n == "minecraft:birch_boat").unwrap() as i32), "minecraft:boat");
    }
}
