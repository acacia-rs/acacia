//! The ride in the log: where the vehicle is and how often the server corrected it, for unattended
//! checks of steering.

use acacia_bot::Bot;
use acacia_bot::proto::types::MetadataDictionaryItemKey as Key;

/// Ticks between log lines while seated.
const EVERY_TICKS: u32 = 10;

/// The yaw of a vehicle that carries the player's view round as it turns: one whose seat locks the
/// rider's rotation (a boat).
pub fn carrying_yaw(bot: &Bot) -> Option<f32> {
    bot.state().riding.seat_turn()?;
    Some(bot.state().entities.get(bot.vehicle()?.runtime_id?)?.yaw)
}

#[derive(Default)]
pub struct RideLog {
    seated_ticks: u32,
    riding: bool,
    /// The server's corrections and teleports of the player so far.
    moved_back: (u32, u32),
    ticks: u32,
    /// When the last tick came, and the longest wait for one since the last log line.
    last: Option<std::time::Instant>,
    longest: u128,
}

impl RideLog {
    /// Call once per tick.
    pub fn tick(&mut self, bot: &Bot) {
        let moved_back = bot.bedrock().and_then(|b| b.movement()).map_or((0, 0), |m| (m.corrections, m.teleports));
        if std::mem::replace(&mut self.moved_back, moved_back) != moved_back {
            let feet = bot.bedrock().and_then(|b| b.movement()).and_then(|m| m.position());
            // The blocks from two below the feet to one above: a correction on the spot is often a block the client's collision gets wrong.
            let column = feet.map(|[x, y, z]| [-2, -1, 0, 1].map(|dy| bot.block_name([x.floor() as i32, y.floor() as i32 + dy, z.floor() as i32]).unwrap_or("?")));
            tracing::info!(target: "moves", corrections = moved_back.0, teleports = moved_back.1, ?feet, ?column, "the server moved the player");
        }
        self.ticks += 1;
        let now = std::time::Instant::now();
        let gap = self.last.replace(now).map_or(0, |last| (now - last).as_millis());
        self.longest = self.longest.max(gap);
        if self.ticks.is_multiple_of(20) {
            tracing::debug!(target: "moves", feet = ?bot.bedrock().and_then(|b| b.movement()).and_then(|m| m.position()), longest_gap_ms = std::mem::take(&mut self.longest), "twenty ticks");
        }
        let Some(vehicle) = bot.vehicle() else {
            if std::mem::take(&mut self.riding) {
                tracing::info!(eye = ?bot.eye_position(), "left the vehicle");
            }
            self.seated_ticks = 0;
            return;
        };
        self.riding = true;
        if self.seated_ticks.is_multiple_of(EVERY_TICKS) {
            let entity = vehicle.runtime_id.and_then(|id| bot.state().entities.get(id));
            let at = entity.map(|e| ([e.position.x, e.position.y, e.position.z], e.yaw));
            let paddles = entity.map(|e| [Key::PaddleTimeLeft, Key::PaddleTimeRight].map(|key| e.metadata.float(key)));
            let (vehicle_corrections, corrections) = bot.bedrock().and_then(|b| b.movement()).map_or((0, 0), |m| (m.vehicle_corrections, m.corrections));
            tracing::info!(kind = vehicle.kind.as_deref(), driver = vehicle.driver, ?at, ?paddles, eye = ?bot.eye_position(), seat = ?bot.state().riding.seat_offset, seat_turn = ?bot.state().riding.seat_turn(), vehicle_corrections, corrections, "riding");
        }
        self.seated_ticks += 1;
    }
}
