//! Physics bots in bed: BDS holds a sleeping player at the bed (0.2 tall box, eye 0.2 above the feet)
//! and corrects any other reported position, so the simulation pauses and the input reports the
//! server's sleeping position.

use acacia_client::proto::types::Vec3f;
use acacia_physics::BlockPos;

use crate::movement::Idle;
use crate::Bot;

/// Per-bot state while a physics bot lies in bed.
#[derive(Default)]
pub(crate) struct Bed {
    /// Stands in for movement while sleeping, continuing its tick count.
    idle: Option<Idle>,
    /// The input position and yaw while sleeping, from the bed the bot clicked.
    pub(crate) lying: Option<(Vec3f, f32)>,
    /// The server's position of the player (feet) when it clicked the bed.
    pub(crate) clicked_from: Option<Vec3f>,
}

/// Where BDS puts a player sleeping in the bed block `pos`: eye position at the
/// clicked block's centre (live: the foot piece clicked, not its head) raised by the bed's height, and
/// the yaw the bed faces.
pub(crate) fn lying_in(name: &str, properties: &str, [x, y, z]: BlockPos) -> Option<(Vec3f, f32)> {
    let height = match name {
        "minecraft:bed" => 0.90625,
        "minecraft:straw_bed" => 0.59375,
        _ => return None,
    };
    let direction = properties.split(',').find_map(|kv| kv.strip_prefix("direction=")?.parse::<u8>().ok())?;
    let yaw = *[0.0, 90.0, -180.0, -90.0].get(usize::from(direction))?;
    Some((Vec3f { x: x as f32 + 0.5, y: y as f32 + height, z: z as f32 + 0.5 }, yaw))
}

impl Bot {
    /// Physics bots: while the server shows the player sleeping, sends a still input at the bed instead
    /// of simulating and returns true. On waking the simulation resumes at the server's position.
    pub(crate) fn bed_physics_tick(&mut self) -> bool {
        let Some(movement) = self.movement.as_mut() else { return false };
        if !self.state.player.is_sleeping() {
            if let Some(idle) = self.bed.idle.take() {
                (self.bed.lying, self.bed.clicked_from) = (None, None);
                let p = &self.state.player.position;
                tracing::debug!(server = ?p, "out of bed");
                movement.resync(idle.last_tick(), [p.x, p.y, p.z], [0.0; 3]);
            }
            return false;
        }
        let idle = self.bed.idle.get_or_insert_with(|| Idle::continuing(movement.input_tick()));
        // BDS moves the player into the bed with a correction on the click tick; that position wins.
        let server = self.state.player.eye_position();
        let moved = self.bed.clicked_from.as_ref() != Some(&self.state.player.position);
        let (eye, yaw) = match self.bed.lying.clone() {
            Some((_, yaw)) if moved => (server, yaw),
            Some(lying) => lying,
            None => (server, self.state.player.yaw),
        };
        if let Some(mut input) = idle.tick_facing(&self.state.player, yaw, 0.0, true) {
            input.position = eye;
            self.add_queued_flags(&mut input);
            self.client.send(&input);
        }
        true
    }
}
