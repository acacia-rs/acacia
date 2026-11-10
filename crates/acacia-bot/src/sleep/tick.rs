//! Physics bots in bed: BDS holds a sleeping player at the bed (0.2 tall box, eye 0.2 above the feet)
//! and corrects any other reported position, so the simulation pauses and the input reports the
//! server's sleeping position.

use acacia_client::proto::types::Vec3f;
use acacia_physics::BlockPos;
use acacia_world::BlockState;
use acacia_world::neutral::value::Direction;
use acacia_world::neutral::{Block, BlockTag, prop};

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
pub(crate) fn lying_in(state: &BlockState, [x, y, z]: BlockPos) -> Option<(Vec3f, f32)> {
    let height = match state.kind()? {
        b if b.is(BlockTag::Beds) => 0.90625,
        Block::StrawBed => 0.59375,
        _ => return None,
    };
    let yaw = match state.get(prop::HorizontalFacing)? {
        Direction::South => 0.0,
        Direction::West => 90.0,
        Direction::North => -180.0,
        _ => -90.0,
    };
    Some((Vec3f { x: x as f32 + 0.5, y: y as f32 + height, z: z as f32 + 0.5 }, yaw))
}

pub(crate) fn is_bed(state: &BlockState) -> bool {
    state.kind().is_some_and(|b| b.is(BlockTag::Beds) || b == Block::StrawBed)
}

impl Bot {
    /// Physics bots: while the server shows the player sleeping, sends a still input at the bed instead
    /// of simulating and returns true. On waking the simulation resumes at the server's position.
    pub(crate) fn bed_physics_tick(&mut self) -> bool {
        let Some(movement) = self.backend.movement_mut() else { return false };
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
            self.send_bedrock(&input);
        }
        true
    }
}
