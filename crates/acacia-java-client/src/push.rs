//! Pushes on the local player from the entities around it. A client ticks every living entity's `aiStep`, whose
//! `pushEntities` only finds the local player there (`EntitySelector.pushableBy`), and every boat's push pass.
// TODO: team collision rules (`Team.CollisionRule`) aren't read; every pusher pushes.

use acacia_java_physics::Aabb;
use acacia_java_proto::entity_types::EntityCollision;

use crate::entity::Entity;
use crate::join::Client;

/// `AbstractBoat.tick`'s push box: `inflate(0.2F, -0.01F, 0.2F)`.
const BOAT_GROW: [f64; 3] = [0.2_f32 as f64, -0.01_f32 as f64, 0.2_f32 as f64];

impl Entity {
    /// Its box when its own tick pushes the player: living entities by their box, boats by a grown one.
    fn push_box(&self) -> Option<Aabb> {
        let (width, height, collision) = self.dimensions()?;
        let bb = Aabb::of_entity(self.position, width, height);
        if crate::riding::is_boat(self.type_name()) {
            let g = BOAT_GROW;
            return Some(Aabb { min: [bb.min[0] - g[0], bb.min[1] - g[1], bb.min[2] - g[2]], max: [bb.max[0] + g[0], bb.max[1] + g[1], bb.max[2] + g[2]] });
        }
        // Bats and armour stands override `pushEntities`; `Pushable` types are the living ones that remain.
        (collision == EntityCollision::Pushable && !self.is("minecraft:bat") && !self.is("minecraft:armor_stand")).then_some(bb)
    }
}

impl Client {
    /// After the player's tick, as the trace replay applies them (the entities tick after it).
    pub(crate) fn push_player(&mut self) {
        let me = self.world.player_id;
        let player = &self.movement.player;
        if self.movement.dead() || !player.pushable(&self.world) {
            return;
        }
        let area = player.bounding_box();
        let pushers: Vec<[f64; 3]> = self
            .world
            .entities
            .iter()
            .filter(|(id, e)| **id != me && !e.passengers.contains(&me) && e.push_box().is_some_and(|b| b.intersects(&area)))
            .map(|(_, e)| e.position)
            .collect();
        for other in pushers {
            self.movement.player.pushed_by(other);
        }
    }
}
